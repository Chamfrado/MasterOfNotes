//! App settings (SPEC-001 R2). One row per key in `settings`, value = JSON. Defaults live here.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{AppError, AppResult};

/// Same enum the scorer uses; serializes as "easy" | "normal" | "hard".
pub use scoring_engine::Difficulty;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReducedMotion {
    System,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeedbackOverlay {
    Full,
    Compact,
    Hidden,
}

/// Last microphone leak check (SPEC-004 R9, T-004-06). Written only by a leak-check run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LeakCheck {
    pub device_id: Option<String>,
    pub video_volume: f64,
    pub passed: bool,
    pub at_ms: i64,
}

/// Volume may rise this much above the checked level and still count as checked.
const LEAK_VOLUME_TOLERANCE: f64 = 0.05;

/// Mirrors `AppSettings` in `apps/desktop/src/lib/ipc.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettings {
    pub library_folder: Option<String>,
    pub recursive_scan: bool,
    pub input_device_id: Option<String>,
    pub noise_floor_dbfs: Option<f64>,
    pub latency_offset_ms: i32,
    pub difficulty: Difficulty,
    pub video_volume: f64,
    pub reduced_motion: ReducedMotion,
    pub feedback_overlay: FeedbackOverlay,
    /// Off by default: the network is used only with consent (Law 10).
    pub auto_check_updates: bool,
    /// Read-only for `update_settings`; written by a leak-check performance run.
    pub leak_check: Option<LeakCheck>,
    /// Larger root scale for TVs (A11Y-03).
    pub tv_mode: bool,
    /// The first-run "check for updates?" consent card was answered.
    pub update_consent_asked: bool,
    /// Installed third-party separation model (SPEC-012). Read-only for `update_settings`.
    pub separation_model: Option<SeparationModel>,
}

/// Written only by `install_separation_model` / `remove_separation_model`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SeparationModel {
    /// SHA-256 hex of the model file (= `KnownModel::id`).
    pub id: String,
    pub name: String,
    pub accepted_ms: i64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            library_folder: None,
            recursive_scan: true,
            input_device_id: None,
            noise_floor_dbfs: None,
            latency_offset_ms: 0,
            difficulty: Difficulty::Normal,
            video_volume: 1.0,
            reduced_motion: ReducedMotion::System,
            feedback_overlay: FeedbackOverlay::Full,
            auto_check_updates: false,
            leak_check: None,
            tv_mode: false,
            update_consent_asked: false,
            separation_model: None,
        }
    }
}

impl AppSettings {
    /// Capture defaults with the calibrated noise floor, when there is one.
    pub fn capture_config(&self) -> audio_engine::CaptureConfig {
        let mut cfg = audio_engine::CaptureConfig::default();
        if let Some(n) = self.noise_floor_dbfs {
            cfg.noise_floor_dbfs = n as f32;
        }
        cfg
    }

    /// Competitive gate (SPEC-006 A2): a passed leak check for the current device,
    /// done at a video volume no lower than the current one (minus a small tolerance).
    pub fn leak_check_ok(&self) -> bool {
        self.leak_check.as_ref().is_some_and(|c| {
            c.passed
                && c.device_id == self.input_device_id
                && self.video_volume <= c.video_volume + LEAK_VOLUME_TOLERANCE
        })
    }

    fn validate(&self) -> AppResult<()> {
        if !(0.0..=1.0).contains(&self.video_volume) {
            return Err(AppError::invalid_setting(
                "O volume do vídeo deve estar entre 0 e 1.",
            ));
        }
        if !(-1000..=1000).contains(&self.latency_offset_ms) {
            return Err(AppError::invalid_setting(
                "O ajuste de latência deve estar entre -1000 e 1000 ms.",
            ));
        }
        if let Some(n) = self.noise_floor_dbfs
            && !(-120.0..=0.0).contains(&n)
        {
            return Err(AppError::invalid_setting(
                "O nível de ruído deve estar entre -120 e 0 dBFS.",
            ));
        }
        if let Some(id) = &self.input_device_id
            && (id.trim().is_empty() || id.len() > 512)
        {
            return Err(AppError::invalid_setting(
                "Identificador de microfone inválido.",
            ));
        }
        Ok(())
    }
}

pub fn load(conn: &Connection) -> AppResult<AppSettings> {
    let Value::Object(mut map) = serde_json::to_value(AppSettings::default())? else {
        return Err(AppError::new("internal", "Configuração padrão inválida."));
    };
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (key, value) = row?;
        // Keys from a newer/older build that this struct doesn't know are ignored.
        if map.contains_key(&key) {
            map.insert(key, serde_json::from_str(&value)?);
        }
    }
    Ok(serde_json::from_value(Value::Object(map))?)
}

/// Applies a partial update. Validates the whole result before writing anything.
/// `libraryFolder` is rejected here: it must go through `set_library_folder`
/// (asset-protocol scope + scan + watcher).
pub fn update(conn: &mut Connection, patch: Map<String, Value>) -> AppResult<AppSettings> {
    if patch.contains_key("libraryFolder") {
        return Err(AppError::invalid_setting(
            "A pasta da biblioteca deve ser escolhida pela tela da biblioteca.",
        ));
    }
    if patch.contains_key("leakCheck") {
        return Err(AppError::invalid_setting(
            "O teste de vazamento é gravado só pelo próprio teste.",
        ));
    }
    if patch.contains_key("separationModel") {
        return Err(AppError::invalid_setting(
            "O modelo de separação é instalado ou removido pela tela de Ajustes.",
        ));
    }
    write(conn, patch)
}

/// Writes keys without the `libraryFolder` guard (used by `set_library_folder`).
pub fn write(conn: &mut Connection, patch: Map<String, Value>) -> AppResult<AppSettings> {
    let Value::Object(mut merged) = serde_json::to_value(load(conn)?)? else {
        return Err(AppError::new("internal", "Configuração inválida."));
    };
    for (key, value) in &patch {
        if !merged.contains_key(key) {
            return Err(AppError::invalid_setting(format!(
                "Configuração desconhecida: {key}."
            )));
        }
        merged.insert(key.clone(), value.clone());
    }
    let settings: AppSettings = serde_json::from_value(Value::Object(merged))
        .map_err(|e| AppError::invalid_setting(format!("Valor de configuração inválido: {e}.")))?;
    settings.validate()?;

    let Value::Object(normalized) = serde_json::to_value(&settings)? else {
        return Err(AppError::new("internal", "Configuração inválida."));
    };
    let tx = conn.transaction()?;
    for key in patch.keys() {
        tx.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, normalized[key].to_string()),
        )?;
    }
    tx.commit()?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("t.db")).unwrap();
        (dir, conn)
    }

    fn patch(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn defaults_when_empty() {
        let (_d, conn) = db();
        assert_eq!(load(&conn).unwrap(), AppSettings::default());
    }

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        {
            let mut conn = crate::db::open(&path).unwrap();
            let s = update(
                &mut conn,
                patch(json!({"videoVolume": 0.5, "difficulty": "hard", "inputDeviceId": "mic-1", "noiseFloorDbfs": -55.5})),
            )
            .unwrap();
            assert_eq!(s.video_volume, 0.5);
        }
        let conn = crate::db::open(&path).unwrap();
        let s = load(&conn).unwrap();
        assert_eq!(s.video_volume, 0.5);
        assert_eq!(s.difficulty, Difficulty::Hard);
        assert_eq!(s.input_device_id.as_deref(), Some("mic-1"));
        assert_eq!(s.noise_floor_dbfs, Some(-55.5));
        assert!(s.recursive_scan, "untouched keys keep defaults");
    }

    #[test]
    fn rejects_invalid() {
        let (_d, mut conn) = db();
        for bad in [
            json!({"videoVolume": 7}),
            json!({"difficulty": "insane"}),
            json!({"latencyOffsetMs": 12.5}),
            json!({"latencyOffsetMs": 5000}),
            json!({"noiseFloorDbfs": 3}),
            json!({"unknownKey": true}),
            json!({"libraryFolder": "C:/x"}),
            json!({"videoVolume": 0.3, "reducedMotion": "maybe"}),
        ] {
            let err = update(&mut conn, patch(bad.clone())).unwrap_err();
            assert_eq!(err.code, "invalid_setting", "{bad}");
        }
        // Nothing was written, not even the valid half of the last patch.
        let n: i64 = conn
            .query_row("SELECT count(*) FROM settings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn null_clears_optional() {
        let (_d, mut conn) = db();
        update(&mut conn, patch(json!({"noiseFloorDbfs": -40}))).unwrap();
        let s = update(&mut conn, patch(json!({"noiseFloorDbfs": null}))).unwrap();
        assert_eq!(s.noise_floor_dbfs, None);
        assert_eq!(load(&conn).unwrap().noise_floor_dbfs, None);
    }

    #[test]
    fn tv_mode_and_update_consent_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        {
            let mut conn = crate::db::open(&path).unwrap();
            let s = load(&conn).unwrap();
            assert!(!s.tv_mode && !s.update_consent_asked, "defaults false");
            update(
                &mut conn,
                patch(json!({"tvMode": true, "updateConsentAsked": true})),
            )
            .unwrap();
            assert_eq!(
                update(&mut conn, patch(json!({"tvMode": "yes"})))
                    .unwrap_err()
                    .code,
                "invalid_setting"
            );
        }
        let s = load(&crate::db::open(&path).unwrap()).unwrap();
        assert!(s.tv_mode && s.update_consent_asked);
    }
}
