// Shared e2e data. Invented titles and names (Law 19): no third-party content.
import type { AppSettings, InputDevice, LeaderboardRow, PerformanceOutcome, Song } from "../src/lib/ipc";
import type { MockSpec } from "./tauri-mock";

export const settings = (libraryFolder: string | null, extra: Partial<AppSettings> = {}): AppSettings => ({
  libraryFolder,
  recursiveScan: true,
  inputDeviceId: null,
  noiseFloorDbfs: null,
  latencyOffsetMs: 0,
  difficulty: "normal",
  videoVolume: 0.8,
  tvMode: false,
  updateConsentAsked: false,
  reducedMotion: "on",
  feedbackOverlay: "full",
  autoCheckUpdates: false,
  leakCheck: null,
  ...extra,
});

export const song = (id: string, title: string, artist: string | null, extra: Partial<Song> = {}): Song => ({
  id,
  path: `C:/Karaoke/${id}.mp4`,
  fileName: `${id}.mp4`,
  title,
  artist,
  extension: "mp4",
  sizeBytes: 1_000_000,
  durationMs: 185_000,
  playable: "unknown",
  available: true,
  referenceStatus: "none",
  ...extra,
});

export const songs: Song[] = [
  song("a1", "Canção da Manhã", "Banda Aurora"),
  song("b2", "Saudade Elétrica", null),
  song("c3", "Pôr do Sol em Óculos", "Zé Mirante", { referenceStatus: "validated" }),
  song("d4", "Arquivo Sumido", null, { available: false }),
];

export const appError = (message: string) => ({ err: { code: "TEST", message } });

const devices: InputDevice[] = [
  { id: "mic-1", name: "Microfone USB", isDefault: true, sampleRate: 48_000, channels: 1 },
  { id: "mic-2", name: "Headset", isDefault: false, sampleRate: 44_100, channels: 2 },
];

const outcome: PerformanceOutcome = {
  performanceId: 7,
  competitive: true,
  result: { score: 87.4, pitch: 0.91, rhythm: 0.8, sustain: 0.86, coverage: 0.9, notesTotal: 40, notesHit: 36, algorithmVersion: 1 },
  exploratory: null,
  personalBest: 80,
  songRecord: 92.1,
};

const leaderboard: LeaderboardRow[] = [
  { rank: 1, participantName: "Lia", score: 92.1, createdMs: 1_763_500_000_000, performanceId: 3 },
  { rank: 2, participantName: "Anônimo", score: 87.4, createdMs: 1_763_600_000_000, performanceId: 7 },
  { rank: 3, participantName: "Rui", score: 71, createdMs: 1_763_400_000_000, performanceId: 1 },
];

/** Every command the existing screens call, answered with plausible data. */
export const fullSpec = (extra: MockSpec = {}): MockSpec => ({
  get_settings: { ok: settings("C:/Karaoke") },
  update_settings: { ok: settings("C:/Karaoke") },
  list_songs: { ok: songs },
  update_song: { ok: songs[0] },
  list_input_devices: { ok: devices },
  start_mic_monitor: { ok: null },
  stop_mic_monitor: { ok: null },
  calibrate_noise: { ok: { noiseFloorDbfs: -62, clippedRatio: 0, issues: [] } },
  get_leaderboard: { ok: leaderboard },
  start_performance: { ok: { competitive: true, reason: null } },
  sync_performance: { ok: null },
  finish_performance: { ok: outcome },
  abort_performance: { ok: null },
  ...extra,
});
