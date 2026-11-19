# Contratos entre módulos

**Dono deste arquivo: Agente 0.** Mudanças só via coordenador (Lei 07). Proponha alterações por mensagem ao coordenador com: motivo, diff proposto, impacto.

## 1. Mapa de propriedade de arquivos

| Caminho | Dono | Observação |
|---|---|---|
| `AGENTS.md`, `CLAUDE.md`, `docs/**`, `specs/**`, `Cargo.toml` (raiz), `package.json` (raiz), `pnpm-workspace.yaml` | Agente 0 | Outros agentes atualizam apenas a seção STATUS da própria spec. |
| `apps/desktop/src/lib/ipc.ts` | Agente 0 | Contrato TS do IPC. Tipos espelham structs Rust (`serde(rename_all="camelCase")`). |
| `apps/desktop/src-tauri/**` (exceto `performance.rs`) | Agente 1 | Inclui `tauri.conf.json`, `capabilities/`, `Cargo.toml` do app, `db/`, `library/`, `commands/`. |
| `apps/desktop/src-tauri/src/performance.rs` | Agente 2 | Sessão ao vivo (âncora, sync, scorer). Registrada no `lib.rs` pelo Agente 1. |
| `crates/audio-engine/**`, `crates/scoring-engine/**`, `tests/fixtures/audio/**`, `tests/fixtures/references/**` | Agente 2 | |
| `apps/desktop/src/**` (exceto `lib/ipc.ts` e `**/*.e2e.*`), `apps/desktop/index.html`, `apps/desktop/vite.config.ts`, estilos, `apps/desktop/package.json` | Agente 3 | Agente 4 pede dependências de teste ao Agente 3. |
| `docs/user/**` (guia do usuário + mídia), `apps/desktop/e2e/docs/**` (scripts de captura de screenshots, GIFs e vídeos) | Agente 5 | Pode importar `e2e/fixtures.ts`/`tauri-mock.ts`/`fake-backend.ts` (só leitura; mudanças via Agente 4). Dependências JS via Agente 3. |
| `crates/game/**`, `apps/desktop/e2e/**`, `apps/desktop/playwright.config.ts`, `.github/workflows/**`, `docs/testing/evidence/**` | Agente 4 | |

## 2. APIs dos crates (Rust)

### 2.1 `audio-engine` (Agente 2)

```rust
pub struct InputDevice { pub id: String, pub name: String, pub is_default: bool, pub sample_rate: u32, pub channels: u16 }
pub fn list_input_devices() -> Result<Vec<InputDevice>, AudioError>;

/// Um frame de análise a cada HOP (10 ms). Volume NÃO influencia pitch/confiança.
pub struct AnalysisFrame {
    pub sample_index: u64,     // índice da 1ª amostra do frame no stream (monotônico)
    pub sample_rate: u32,
    pub f0_hz: Option<f32>,    // None = sem pitch detectável (silêncio, ruído, fricativa)
    pub confidence: f32,       // 0..1 (YIN: 1 - aperiodicidade)
    pub rms_dbfs: f32,
    pub voiced: bool,          // VAD: acima do noise gate E com pitch confiável
    pub clipped: bool,
}
/// Captura: análise roda em thread própria, nunca no callback de áudio. Drop = parar.
pub struct Capture;
impl Capture {
    pub fn start(device_id: Option<&str>, cfg: CaptureConfig, on_frame: impl FnMut(AnalysisFrame) + Send + 'static) -> Result<Capture, AudioError>;
}
pub struct CaptureConfig { pub noise_floor_dbfs: f32 /* do calibrate */, pub gate_margin_db: f32 } // Default: -60 / 10
/// Função pura também exposta para testes/offline: analisa um buffer mono.
pub fn analyze_buffer(samples: &[f32], sample_rate: u32, cfg: &CaptureConfig) -> Vec<AnalysisFrame>;
/// Tempo do frame = centro da janela: sample_index + window_len(sr)/2.
pub fn window_len(sample_rate: u32) -> usize;
pub fn hop_len(sample_rate: u32) -> usize;
pub const YIN_THRESHOLD, MIN_F0_HZ, MAX_F0_HZ, MIN_CONFIDENCE, CLIP_LEVEL, SILENCE_DBFS /* -120 */;
pub enum AudioError { Device(String), /* ... */ }        // thiserror, Display pt-BR
impl Capture { pub fn take_error(&self) -> Option<AudioError>; }
impl Capture { pub fn captured_samples(&self) -> u64; /* contador do callback (âncora) */ pub fn start_raw(device_id: Option<&str>, on_samples: impl FnMut(u64, &[f32]) + Send + 'static) -> Result<Capture, AudioError>; /* sem análise; T-004-07 */ }
// Latência (T-004-07), puras:
pub struct LatencyResult { pub offset_ms: f32, pub jitter_ms: f32, pub detected: u32, pub expected: u32 }
pub fn detect_onsets(samples: &[f32], sample_rate: u32, noise_floor_dbfs: f32) -> Vec<f64 /*ms*/>;
pub fn latency_from(onsets_ms: &[f64], clicks_ms: &[f64]) -> LatencyResult; // ex.: dispositivo desconectado (Lei 13). Capture: Send.

// Calibração (funções puras sobre frames)
pub enum CalibrationIssue { Noisy, Clipping, TooQuiet }
pub struct NoiseCalibration { pub noise_floor_dbfs: f32 /* p90 do RMS */, pub clipped_ratio: f32, pub issues: Vec<CalibrationIssue> }
pub fn calibrate_noise(frames: &[AnalysisFrame]) -> NoiseCalibration;
pub fn check_voice(frames: &[AnalysisFrame], cfg: &CaptureConfig) -> Vec<CalibrationIssue>;
```
Tipos públicos com `Serialize` em camelCase.

### 2.2 `scoring-engine` (Agente 2)

```rust
pub const ALGORITHM_VERSION: u32 = 1;          // bump a cada mudança que altere notas
pub const REFERENCE_FORMAT_VERSION: u32 = 1;

pub struct Note { pub start_ms: u32, pub dur_ms: u32, pub midi: f32, pub lyric: Option<String> }
pub struct MelodyReference { pub format_version: u32, pub notes: Vec<Note> } // ordenadas, sem sobreposição
pub fn parse_reference_json(json: &str) -> Result<MelodyReference, RefError>;
pub fn import_midi(bytes: &[u8], track: Option<usize>) -> Result<MelodyReference, RefError>;
pub fn midi_tracks(bytes: &[u8]) -> Result<Vec<MidiTrackInfo>, RefError>;  // para o usuário escolher a trilha vocal
pub fn to_reference_json(r: &MelodyReference) -> String;  // inverso do parse; formato de melody_references.notes_json
pub struct MidiTrackInfo { pub index: usize, pub name: Option<String>, pub note_count: u32 }
/// Display pt-BR pronto para o usuário. import_midi(None) com várias trilhas → TrackRequired.
pub enum RefError { Json, UnknownFormat, UnsupportedVersion, Empty, ZeroDuration{..}, PitchOutOfRange{..}, Midi, SmpteUnsupported, SequentialUnsupported, NoSuchTrack, TrackRequired }

pub enum Difficulty { Easy, Normal, Hard }   // serde: "easy" | "normal" | "hard"
pub struct VoiceFrame { pub t_ms: f64 /* tempo de música */, pub f0_hz: Option<f32>, pub confidence: f32, pub voiced: bool }
pub struct Scorer;
impl Scorer {
    pub fn new(reference: &MelodyReference, offset_ms: i32, difficulty: Difficulty) -> Self;
    pub fn push(&mut self, frame: VoiceFrame) -> Option<LiveFeedback>; // Some quando há nota ativa
    pub fn finish(self) -> ScoreResult;                                // determinístico
}
pub enum FeedbackLabel { Excellent, VeryGood, Good, NeedsWork, NotDetected }
pub struct LiveFeedback { pub t_ms: f64, pub target_midi: f32, pub sung_midi: Option<f32>, pub cents_error: Option<f32>, pub label: FeedbackLabel, pub running_score: f32 }
pub struct ScoreResult { pub score: f32 /*0..100*/, pub pitch: f32, pub rhythm: f32, pub sustain: f32, pub coverage: f32, pub notes_total: u32, pub notes_hit: u32, pub algorithm_version: u32 }
/// SPEC-006 item 9: sem referência, só métricas exploratórias (não competitivas, nunca vão para ranking).
pub struct ExploratoryMetrics { pub voiced_ratio: f32 /* 0..1 */, pub pitch_stability: f32 /* 0..1: 1 - min(1, desvio absoluto mediano em cents dentro de trechos sustentados / 100) */ }
pub fn exploratory_metrics(frames: &[VoiceFrame]) -> ExploratoryMetrics;
```
Tipos públicos com `Serialize` em camelCase (`FeedbackLabel`: `"veryGood"`, `"notDetected"`…).

### 2.3 `game` (Agente 4) — puro, sem I/O

```rust
// Tipos com #[serde(rename_all = "camelCase")]. Erros: thiserror.
pub enum GameMode { Solo, Party, Duel, Challenge }
pub enum RoundOrder { Manual, Shuffled }
pub struct PerformanceRecord { pub participant_id: Option<i64> /* None = Anônimo (Solo) */, pub song_id: String, pub score: f32, pub compat: CompatKey, pub created_ms: i64 }
pub struct CompatKey { pub reference_id: i64, pub reference_revision: i64, pub algorithm_version: u32, pub difficulty: String }
pub struct Standing { pub participant_id: i64, pub total: f32, pub average: f32, pub rounds: u32, pub eligible: bool, pub rank: u32 }
pub struct DuelOutcome { pub winner: Option<i64> /* None = empate */, pub margin: f32 }
pub enum ChallengeStatus { InProgress, Succeeded, Failed }
pub struct ChallengeProgress { pub completed: u32, pub remaining: u32, pub average: f32, pub status: ChallengeStatus }
pub struct LeaderboardRow { pub position: u32, pub participant_id: Option<i64>, pub score: f32, pub created_ms: i64 }
pub enum GameError { IncompatibleRecords }

/// Ordena por média entre elegíveis; desempate: total, depois id. Ignora registros sem participante.
pub fn party_standings(records: &[PerformanceRecord], min_rounds: u32) -> Vec<Standing>;
/// max(nP, nS) rodadas; rodada i = (part[i % nP], song[i % nS]). Shuffled: Fisher-Yates + splitmix64 por seed nas duas listas.
pub fn plan_rounds(participants: &[i64], songs: &[String], order: RoundOrder, seed: u64) -> Vec<(i64, String)>;
pub fn duel_outcome(a: &PerformanceRecord, b: &PerformanceRecord) -> Result<DuelOutcome, GameError>; // erro se CompatKey difere
pub fn challenge_progress(scores: &[f32], target: f32, rounds: u32) -> ChallengeProgress;
/// Filtra pela CompatKey; o chamador já filtrou competitive=1. Empate: mais antigo primeiro. Top 50.
pub fn leaderboard(records: &[PerformanceRecord], compat: &CompatKey) -> Vec<LeaderboardRow>;
```

### 2.4 `src-tauri/src/performance.rs` (Agente 2) — sessão ao vivo, sem acesso ao banco

```rust
pub struct PerformanceConfig { pub reference: Option<(MelodyReference, i32 /*offset_ms*/)>, pub competitive: bool /* ref validated */, pub difficulty: Difficulty, pub latency_offset_ms: f64, pub device_id: Option<String>, pub capture: CaptureConfig }
pub struct MicFrame { rms_dbfs, f0_hz, confidence, voiced, clipped }   // camelCase; From<&AnalysisFrame>; reusado pelo start_mic_monitor
pub struct LiveTick { pub mic: MicFrame, pub feedback: Option<LiveFeedback> } // enviado à UI a ~30 Hz; o Scorer recebe todos os frames
pub struct LivePerformance;
impl LivePerformance {
    /// Abre o mic; frames são descartados até o 1º sync(playing=true).
    pub fn start(cfg: PerformanceConfig, on_tick: impl Fn(LiveTick) + Send + 'static) -> Result<Self, AppError>;
    /// Âncora = (video_time, contador de amostras do CALLBACK). Resync se deriva > 40 ms; pausa descarta; seek → competitive=false + Scorer::skip.
    pub fn sync(&self, video_time_s: f64, playing: bool, seeked: bool);
    pub fn take_error(&self) -> Option<AppError>;   // mic desconectado
    pub fn finish(self) -> PerformanceSummary;      // drop = abort
}
pub struct PerformanceSummary { pub competitive: bool, pub reason: Option<String> /* pt-BR, por que virou treino (v3) */, pub played_ms: u64, pub result: Option<ScoreResult>, pub exploratory: Option<ExploratoryMetrics>, pub capture: CaptureStats }
// CaptureStats → performances.capture_json: device, noiseFloorDbfs, latencyOffsetMs, clippedRatio, missingRatio (0..1; > 0,05 ⇒ treino, v3 N2), leakSuspected (só indicativo)
```

Os commands (Agente 1) carregam a referência ativa e as configurações, criam `PerformanceConfig`, guardam `Mutex<Option<LivePerformance>>` no `AppState`, repassam `LiveTick` ao `Channel` e, no `finish`, decidem a persistência: `competitive` final = `summary.competitive && played_ms ≥ 50% de duration_ms` (SPEC-007 R2). Também calculam os recordes.
**Checagem de vazamento (T-004-06, SPEC-006 A2):** `start_performance` com `leakCheck: true` roda uma performance curta com o usuário calado. Nada é persistido em `performances`. `PerformanceConfig` ganha `leak_check: bool`, e `PerformanceSummary` ganha `leak: Option<LeakVerdict { passed, guide_score }>`, onde `guide_score` é a componente de pitch P medida nas notas tocadas. O limiar de `passed` é definido pelo Agente 2 com evidência e documentado na SPEC-004. O command grava `settings.leakCheck = { deviceId, videoVolume, passed, atMs }`. A partir daí, `competitive` também exige `leakCheck.passed && leakCheck.deviceId == inputDeviceId && videoVolume ≤ leakCheck.videoVolume + 0,05`; senão é treino, com `reason` em pt-BR sugerindo fones ou o teste.
`LeakVerdict { passed, guide_score, reason: Option<String> }`: limiar `guide_score ≤ 0,05`, com ≥ 8 s e ≥ 3 notas inteiras (evidência na SPEC-004 R9).
**Sonda de latência (T-004-07):** `PerformanceConfig.latency_probe: Option<Vec<f64>>` (instantes dos cliques em tempo de vídeo). É exclusiva com `leak_check` e exige `latency_offset_ms = 0`; se não, `invalid_input`. Usa `Capture::start_raw` (sem LiveTick) e guarda as amostras só a partir do 1º `sync(playing=true)`. `PerformanceSummary.latency: Option<LatencyVerdict { LatencyResult (flatten), passed, reason }>`. offset = mediana(onset_t_song − click_t), com o histórico de âncoras. Falha se detected < expected/2, se o offset sair de ±500 ms, se houver seek ou se forem < 4 cliques. Se `passed`, o command grava `settings.latencyOffsetMs = round(offset)`. A UI toca a trilha de cliques (WAV gerado em JS, 8 × 750 ms, 1º clique ≥ 500 ms) pelo mesmo pipeline de mídia do vídeo.
`scoring-engine` ganha `Scorer::skip(from_ms, to_ms)`: exclui da agregação as notas que começam no trecho pulado (só em treino; não altera `ALGORITHM_VERSION`).

## 3. Esquema SQLite (migração 001)

Dono da implementação: Agente 1 (`src-tauri/migrations/001_init.sql`). Mudanças de esquema = nova migração, nunca editar migração já integrada.

```sql
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);           -- value = JSON
CREATE TABLE songs (
  id TEXT PRIMARY KEY,               -- content id: blake3(size ‖ primeiros 64KiB ‖ últimos 64KiB), 32 primeiros caracteres hex (128 bits)
  path TEXT NOT NULL UNIQUE,
  file_name TEXT NOT NULL,
  title TEXT NOT NULL,               -- inicial = nome do arquivo sem extensão
  artist TEXT,
  extension TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  modified_ms INTEGER NOT NULL,
  duration_ms INTEGER,               -- preenchido pela UI em loadedmetadata
  playable TEXT NOT NULL DEFAULT 'unknown' CHECK (playable IN ('unknown','yes','no')),
  available INTEGER NOT NULL DEFAULT 1,  -- 0 = arquivo sumiu; linha mantida (histórico)
  added_ms INTEGER NOT NULL,
  updated_ms INTEGER NOT NULL
);
CREATE TABLE melody_references (
  id INTEGER PRIMARY KEY,
  song_id TEXT NOT NULL REFERENCES songs(id),
  source TEXT NOT NULL CHECK (source IN ('midi','json','manual','extracted')),
  format_version INTEGER NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1,  -- +1 a cada mudança de notas ou offset
  status TEXT NOT NULL CHECK (status IN ('processing','experimental','validated','invalid')),
  offset_ms INTEGER NOT NULL DEFAULT 0,
  notes_json TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 1,    -- no máximo 1 ativa por música
  created_ms INTEGER NOT NULL,
  updated_ms INTEGER NOT NULL
);
CREATE UNIQUE INDEX one_active_reference ON melody_references(song_id) WHERE active = 1;
CREATE TABLE participants (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE, created_ms INTEGER NOT NULL);
CREATE TABLE game_sessions (
  id INTEGER PRIMARY KEY,
  mode TEXT NOT NULL CHECK (mode IN ('solo','party','duel','challenge')),
  name TEXT,
  status TEXT NOT NULL CHECK (status IN ('active','completed','abandoned')),
  config_json TEXT NOT NULL DEFAULT '{}',   -- minRounds, targetScore, difficulty, seed
  started_ms INTEGER NOT NULL,
  completed_ms INTEGER
);
CREATE TABLE rounds (
  id INTEGER PRIMARY KEY,
  session_id INTEGER NOT NULL REFERENCES game_sessions(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  song_id TEXT NOT NULL REFERENCES songs(id),
  participant_id INTEGER REFERENCES participants(id),
  status TEXT NOT NULL CHECK (status IN ('pending','completed','skipped')),
  UNIQUE (session_id, position)
);
CREATE TABLE performances (
  id INTEGER PRIMARY KEY,
  round_id INTEGER REFERENCES rounds(id),
  song_id TEXT NOT NULL REFERENCES songs(id),
  participant_id INTEGER REFERENCES participants(id),
  reference_id INTEGER REFERENCES melody_references(id),
  reference_revision INTEGER,
  algorithm_version INTEGER NOT NULL,
  difficulty TEXT NOT NULL,
  competitive INTEGER NOT NULL,       -- 1 só se referência validated e sem seek
  score REAL NOT NULL,
  metrics_json TEXT NOT NULL,         -- ScoreResult completo
  capture_json TEXT NOT NULL,         -- dispositivo, noise floor, latency, clipping %
  created_ms INTEGER NOT NULL
);
CREATE INDEX perf_leaderboard ON performances(song_id, reference_id, reference_revision, algorithm_version, difficulty, score DESC) WHERE competitive = 1;
```

Áudio do microfone **nunca** é persistido (Lei 14).

## 4. IPC (Tauri commands e eventos)

Fonte de verdade: [`apps/desktop/src/lib/ipc.ts`](../../apps/desktop/src/lib/ipc.ts). Todos os comandos retornam `Result<T, AppError>` em Rust; `AppError` serializa como `{ code: string, message: string }`, mensagem em pt-BR pronta para o usuário.

Regras:
- Nome do comando Rust = nome snake_case da função em `ipc.ts`.
- Structs com `#[serde(rename_all = "camelCase")]`.
- Streams de alta frequência usam `tauri::ipc::Channel<T>`, não eventos globais.
- Eventos globais: `library://changed` (payload `LibraryChange`); `audio://error` (payload `AppError`): emitido quando o dispositivo falha durante o medidor ou a performance; o stream já está parado.
