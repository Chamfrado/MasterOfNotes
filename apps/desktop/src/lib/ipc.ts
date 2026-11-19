// IPC contract between the React UI and the Rust core.
// OWNER: Agent 0 (coordinator). Changes go through docs/architecture/contracts.md (Law 07).
// Every function maps 1:1 to a #[tauri::command] with the snake_case name.
import { Channel, convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Error shape returned by every command. `message` is user-facing pt-BR. */
export interface AppError {
  code: string;
  message: string;
}

// ---------- Settings (SPEC-001) ----------
export type Difficulty = "easy" | "normal" | "hard";
export interface AppSettings {
  libraryFolder: string | null;
  recursiveScan: boolean;
  inputDeviceId: string | null;
  noiseFloorDbfs: number | null; // null = not calibrated
  latencyOffsetMs: number; // mic input + video output latency
  difficulty: Difficulty;
  videoVolume: number; // 0..1
  reducedMotion: "system" | "on" | "off";
  feedbackOverlay: "full" | "compact" | "hidden";
  autoCheckUpdates: boolean;
  /** Larger root scale for TVs (A11Y-03). Default false. */
  tvMode: boolean;
  /** The first-run "check for updates?" consent card was answered. Default false. */
  updateConsentAsked: boolean;
  /** Last leak check (SPEC-004 R9 / T-004-06). Competitive play requires a passed check for the current device at <= this volume. Read-only via updateSettings. */
  leakCheck: LeakCheck | null;
}
export interface LeakCheck {
  deviceId: string | null;
  videoVolume: number;
  passed: boolean;
  atMs: number;
}
export const getSettings = () => invoke<AppSettings>("get_settings");
export const updateSettings = (patch: Partial<AppSettings>) =>
  invoke<AppSettings>("update_settings", { patch });

// ---------- Library (SPEC-002) ----------
export type Playable = "unknown" | "yes" | "no";
export type ReferenceStatus = "none" | "processing" | "experimental" | "validated" | "invalid";
export interface Song {
  id: string;
  path: string;
  fileName: string;
  title: string;
  artist: string | null;
  extension: string;
  sizeBytes: number;
  durationMs: number | null;
  playable: Playable;
  available: boolean;
  referenceStatus: ReferenceStatus;
}
export interface ScanSummary {
  found: number;
  added: number;
  updated: number;
  removed: number;
  skipped: number; // unsupported extension or unreadable
  durationMs: number;
}
export interface LibraryChange {
  added: string[];
  updated: string[];
  removed: string[];
}
/** Persists the folder, allows it in the asset-protocol scope, scans it and starts the watcher. */
export const setLibraryFolder = (path: string, recursive: boolean) =>
  invoke<ScanSummary>("set_library_folder", { path, recursive });
export const rescanLibrary = () => invoke<ScanSummary>("rescan_library");
export const listSongs = () => invoke<Song[]>("list_songs");
export const updateSong = (
  id: string,
  patch: { title?: string; artist?: string | null; durationMs?: number; playable?: Playable },
) => invoke<Song>("update_song", { id, patch });
export const onLibraryChanged = (cb: (c: LibraryChange) => void): Promise<UnlistenFn> =>
  listen<LibraryChange>("library://changed", (e) => cb(e.payload));
/** URL for <video src>. Works only for files inside the library folder. */
export const videoSrc = (song: Song) => convertFileSrc(song.path);

// ---------- Microphone (SPEC-004) ----------
export interface InputDevice {
  id: string;
  name: string;
  isDefault: boolean;
  sampleRate: number;
  channels: number;
}
export interface MicFrame {
  rmsDbfs: number;
  f0Hz: number | null;
  confidence: number;
  voiced: boolean;
  clipped: boolean;
}
export type CalibrationIssue = "noisy" | "clipping" | "tooQuiet";
export interface NoiseCalibration {
  noiseFloorDbfs: number;
  clippedRatio: number;
  issues: CalibrationIssue[]; // empty = ok
}
export const listInputDevices = () => invoke<InputDevice[]>("list_input_devices");
/** Live meter for the mic setup screen (~30 Hz). Stop before starting a performance. */
export const startMicMonitor = (deviceId: string | null, onFrame: (f: MicFrame) => void) => {
  const ch = new Channel<MicFrame>();
  ch.onmessage = onFrame;
  return invoke<void>("start_mic_monitor", { deviceId, onFrame: ch });
};
export const stopMicMonitor = () => invoke<void>("stop_mic_monitor");
/** Device lost/failed during the monitor or a performance (Law 13). The stream is already stopped. */
export const onAudioError = (cb: (e: AppError) => void): Promise<UnlistenFn> =>
  listen<AppError>("audio://error", (e) => cb(e.payload));
/** Records `seconds` of room noise (user silent). Saves noiseFloorDbfs in settings. */
export const calibrateNoise = (deviceId: string | null, seconds: number) =>
  invoke<NoiseCalibration>("calibrate_noise", { deviceId, seconds });

// ---------- Melody reference (SPEC-005) ----------
export interface RefNote {
  startMs: number;
  durMs: number;
  midi: number;
  lyric: string | null;
}
export interface MelodyReference {
  id: number;
  songId: string;
  source: "midi" | "json" | "manual" | "extracted";
  status: Exclude<ReferenceStatus, "none">;
  revision: number;
  offsetMs: number;
  notes: RefNote[];
}
export interface MidiTrackInfo {
  index: number;
  name: string | null;
  noteCount: number;
}
export const getReference = (songId: string) =>
  invoke<MelodyReference | null>("get_reference", { songId });
export const listMidiTracks = (filePath: string) =>
  invoke<MidiTrackInfo[]>("list_midi_tracks", { filePath });
/** Imports .mid/.midi/.json. Result status is always "experimental" until validated by the user. */
export const importReference = (songId: string, filePath: string, midiTrack: number | null) =>
  invoke<MelodyReference>("import_reference", { songId, filePath, midiTrack });
export const setReferenceOffset = (referenceId: number, offsetMs: number) =>
  invoke<MelodyReference>("set_reference_offset", { referenceId, offsetMs });
/** User confirmed alignment by ear/eye. Only validated references allow competitive scores. */
export const setReferenceStatus = (referenceId: number, status: "experimental" | "validated" | "invalid") =>
  invoke<MelodyReference>("set_reference_status", { referenceId, status });

// ---------- Live performance (SPEC-006, sync: docs/architecture/overview.md) ----------
export type FeedbackLabel = "excellent" | "veryGood" | "good" | "needsWork" | "notDetected";
export interface LiveFeedback {
  tMs: number;
  targetMidi: number;
  sungMidi: number | null;
  centsError: number | null;
  label: FeedbackLabel;
  runningScore: number;
}
export interface LiveTick {
  mic: MicFrame;
  feedback: LiveFeedback | null; // null outside notes (instrumental) or without reference
}
export interface StartPerformance {
  songId: string;
  roundId: number | null;
  participantId: number | null;
  /** true = leak check run: user stays silent, nothing is persisted, outcome.leak is set and settings.leakCheck is saved. */
  leakCheck?: boolean;
  /** Latency probe (T-004-07): click times in VIDEO time (ms) of the click track the UI plays. Exclusive with leakCheck. */
  latencyProbe?: number[];
}
export interface PerformanceStarted {
  competitive: boolean; // false when no validated reference
  reason: string | null; // pt-BR, why not competitive
}
export interface ScoreResult {
  score: number;
  pitch: number;
  rhythm: number;
  sustain: number;
  coverage: number;
  notesTotal: number;
  notesHit: number;
  algorithmVersion: number;
}
/** Without a reference: informative only, never ranked (SPEC-006 item 9). */
export interface ExploratoryMetrics {
  voicedRatio: number; // 0..1
  pitchStability: number; // 0..1
}
export interface PerformanceOutcome {
  performanceId: number | null; // null when nothing was persisted (no reference)
  competitive: boolean;
  result: ScoreResult | null; // null when there was no reference
  exploratory: ExploratoryMetrics | null; // set when result is null
  leak?: { passed: boolean; guideScore: number; reason: string | null } | null; // only on leak check runs
  latency?: LatencyVerdict | null; // only on latency probe runs; if passed, settings.latencyOffsetMs was updated
  personalBest: number | null;
  songRecord: number | null;
}
export interface LatencyVerdict {
  offsetMs: number;
  jitterMs: number;
  detected: number;
  expected: number;
  passed: boolean;
  reason: string | null; // pt-BR
}
export const startPerformance = (args: StartPerformance, onTick: (t: LiveTick) => void) => {
  const ch = new Channel<LiveTick>();
  ch.onmessage = onTick;
  return invoke<PerformanceStarted>("start_performance", { args, onTick: ch });
};
/** Call on play/pause/seeking/seeked and every ~250ms with video.currentTime (mediaTime). */
export const syncPerformance = (videoTimeS: number, playing: boolean, seeked: boolean) =>
  invoke<void>("sync_performance", { videoTimeS, playing, seeked });
export const finishPerformance = () => invoke<PerformanceOutcome>("finish_performance");
export const abortPerformance = () => invoke<void>("abort_performance");

// ---------- Participants, sessions, rankings (SPEC-007, SPEC-008) ----------
export type GameMode = "solo" | "party" | "duel" | "challenge";
export interface Participant {
  id: number;
  name: string;
}
export interface Round {
  id: number;
  position: number;
  songId: string;
  participantId: number | null;
  status: "pending" | "completed" | "skipped";
  score: number | null;
}
export interface Standing {
  participantId: number;
  total: number;
  average: number;
  rounds: number;
  eligible: boolean;
  rank: number;
}
export interface GameSession {
  id: number;
  mode: GameMode;
  name: string | null;
  status: "active" | "completed" | "abandoned";
  config: { minRounds?: number; targetScore?: number; difficulty: Difficulty; seed: number };
  startedMs: number;
  completedMs: number | null;
  rounds: Round[];
  standings: Standing[];
}
export interface CreateSession {
  mode: GameMode;
  name: string | null;
  participantIds: number[];
  songIds: string[];
  order: "manual" | "shuffle";
  minRounds?: number;
  targetScore?: number;
}
export interface LeaderboardRow {
  rank: number;
  participantName: string;
  score: number;
  createdMs: number;
  performanceId: number;
}
export const listParticipants = () => invoke<Participant[]>("list_participants");
export const createParticipant = (name: string) => invoke<Participant>("create_participant", { name });
export const createSession = (args: CreateSession) => invoke<GameSession>("create_session", { args });
export const getSession = (id: number) => invoke<GameSession>("get_session", { id });
export const listSessions = () => invoke<GameSession[]>("list_sessions");
export const setRoundStatus = (roundId: number, status: "pending" | "skipped") =>
  invoke<GameSession>("set_round_status", { roundId, status });
export const completeSession = (id: number) => invoke<GameSession>("complete_session", { id });
/** Ranking for the song's current active reference revision + current algorithm + difficulty. */
export const getLeaderboard = (songId: string, difficulty: Difficulty) =>
  invoke<LeaderboardRow[]>("get_leaderboard", { songId, difficulty });

/** Performance history incl. older algorithm/reference versions (SPEC-007 R6). Newest first. */
export interface PerformanceRow {
  performanceId: number;
  songId: string;
  songTitle: string;
  participantName: string | null; // null = Anônimo
  score: number;
  competitive: boolean;
  algorithmVersion: number;
  referenceRevision: number;
  difficulty: Difficulty;
  createdMs: number;
  /** Same CompatKey as the song's current one (active reference + revision + ALGORITHM_VERSION), for this row's difficulty. false = "versão anterior". */
  isCurrentKey: boolean;
}
export interface ListPerformances {
  songId?: string;
  participantId?: number;
  limit: number; // 1..500
}
export const listPerformances = (args: ListPerformances) =>
  invoke<PerformanceRow[]>("list_performances", { args });

// Updates (SPEC-009) use @tauri-apps/plugin-updater directly; no custom commands.
