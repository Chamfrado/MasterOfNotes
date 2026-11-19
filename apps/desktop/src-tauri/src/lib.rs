mod commands;
mod db;
mod error;
mod library;
mod live;
mod mic;
mod performance;
mod reference;
mod sessions;
mod settings;

use tauri::Manager;

/// The updater only runs with a real signing key in the config (SPEC-009 R3). Without one the
/// app keeps working offline and the UI's update check fails with "plugin not found" (R7).
fn updater_pubkey<R: tauri::Runtime>(context: &tauri::Context<R>) -> Option<&str> {
    context
        .config()
        .plugins
        .0
        .get("updater")?
        .get("pubkey")?
        .as_str()
        .filter(|k| !k.trim().is_empty())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    let updater = updater_pubkey(&context).is_some();
    let mut builder = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());
    if updater {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }
    builder
        .setup(move |app| {
            if !updater {
                log::warn!("updater desativado: sem chave pública na configuração");
            }
            let db = app
                .path()
                .app_data_dir()
                .map_err(|e| {
                    error::AppError::new(
                        "db_open_failed",
                        format!("Pasta de dados do aplicativo indisponível: {e}"),
                    )
                })
                .and_then(|dir| {
                    std::fs::create_dir_all(&dir)?;
                    db::open(&dir.join("masterofnotes.db"))
                });
            if let Err(e) = &db {
                log::error!("database unavailable: {e}");
            }
            app.manage(commands::AppState::new(db));
            app.manage(mic::MicMonitor::default());
            app.manage(live::LiveState::default());
            commands::restore_library(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::set_library_folder,
            commands::rescan_library,
            commands::list_songs,
            commands::update_song,
            mic::list_input_devices,
            mic::start_mic_monitor,
            mic::stop_mic_monitor,
            mic::calibrate_noise,
            reference::get_reference,
            reference::list_midi_tracks,
            reference::import_reference,
            reference::set_reference_offset,
            reference::set_reference_status,
            sessions::cmd::list_participants,
            sessions::cmd::create_participant,
            sessions::cmd::create_session,
            sessions::cmd::get_session,
            sessions::cmd::list_sessions,
            sessions::cmd::set_round_status,
            sessions::cmd::complete_session,
            sessions::cmd::get_leaderboard,
            live::start_performance,
            live::sync_performance,
            live::finish_performance,
            live::abort_performance,
        ])
        .run(context)
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updater_needs_a_real_key() {
        let mut ctx: tauri::Context<tauri::test::MockRuntime> =
            tauri::test::mock_context(tauri::test::noop_assets());
        assert_eq!(updater_pubkey(&ctx), None, "no updater section");
        let plugins = &mut ctx.config_mut().plugins.0;
        plugins.insert("updater".into(), serde_json::json!({"pubkey": "  "}));
        assert_eq!(updater_pubkey(&ctx), None, "blank key");
        let plugins = &mut ctx.config_mut().plugins.0;
        plugins.insert(
            "updater".into(),
            serde_json::json!({"pubkey": "dW50cnVzdGVk"}),
        );
        assert_eq!(updater_pubkey(&ctx), Some("dW50cnVzdGVk"));
    }
}
