// T-003-05: self-authored test song for validating the real app (Law 19: no third-party content, no ffmpeg).
// The mon-melody JSON next to this file is the single source of truth: audio and video are rendered from it,
// so the exported reference matches the media by construction (alignment measured in e2e/test-song.e2e.ts).
import type { Page } from "@playwright/test";
import melody from "./test-song.mon-melody.json" with { type: "json" };

export const SONG_MS = 31_000;
export const MELODY_JSON = melody;
export const FILES = {
  guide: "mon-test-song.webm",
  karaoke: "mon-test-song-karaoke.webm",
  reference: "mon-test-song.mon-melody.json",
} as const;

interface Note {
  startMs: number;
  durMs: number;
  midi: number;
  lyric?: string;
}

interface RenderArgs {
  notes: Note[];
  songMs: number;
  guide: boolean;
  fileName: string;
}

/** Runs inside the page: WebAudio + canvas → MediaRecorder (real time), then downloads the WebM. */
async function renderSong({ notes, songMs, guide, fileName }: RenderArgs) {
  const W = 1280;
  const H = 720;
  const hz = (m: number) => 440 * 2 ** ((m - 69) / 12);
  const NAMES = ["Dó", "Dó♯", "Ré", "Ré♯", "Mi", "Fá", "Fá♯", "Sol", "Sol♯", "Lá", "Lá♯", "Si"];
  const noteName = (m: number) => `${NAMES[Math.round(m) % 12]}${Math.floor(Math.round(m) / 12) - 1}`;
  // Low pure-sine pad (MIDI ≤ 55): never shares a frequency with the guide (MIDI 60–72).
  const CHORDS: Record<string, number[]> = { C: [48, 52, 55], F: [41, 45, 48], G: [43, 47, 50], Am: [45, 48, 52] };
  const BARS = ["C", "C", "F", "G", "C", "F", "C", "Am", "C", "F", "G", "G", "Am", "F", "C", "C"];

  const ac = new AudioContext({ sampleRate: 48_000 });
  await ac.resume();
  const dest = ac.createMediaStreamDestination();
  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = H;
  const g = canvas.getContext("2d");
  if (!g) throw new Error("canvas 2d unavailable");
  const stream = new MediaStream([...canvas.captureStream(30).getVideoTracks(), ...dest.stream.getAudioTracks()]);
  // A keyframe every second: without it, seeking shows a stale frame (measured; the file has no Cues).
  const options: MediaRecorderOptions & { videoKeyFrameIntervalDuration: number } = {
    mimeType: "video/webm;codecs=vp8,opus",
    videoBitsPerSecond: 2_000_000,
    audioBitsPerSecond: 128_000,
    videoKeyFrameIntervalDuration: 1000,
  };
  const rec = new MediaRecorder(stream, options);
  const chunks: Blob[] = [];
  rec.ondataavailable = (e) => chunks.push(e.data);

  // Phrases (for the lyric line) split at rests ≥ 1 s.
  const phrases: Note[][] = [];
  notes.forEach((n, i) => {
    const prev = notes[i - 1];
    if (!prev || n.startMs - (prev.startMs + prev.durMs) >= 1000) phrases.push([]);
    phrases[phrases.length - 1].push(n);
  });
  const clock = (ms: number) => `${Math.floor(ms / 60_000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, "0")}`;

  const draw = () => {
    const t = (ac.currentTime - t0) * 1000;
    const cur = notes.find((n) => t >= n.startMs && t < n.startMs + n.durMs);
    const next = notes.find((n) => n.startMs > t);
    g.fillStyle = "#14141c";
    g.fillRect(0, 0, W, H);
    g.fillStyle = "#f4f4f8";
    g.font = "bold 40px sans-serif";
    g.textAlign = "left";
    g.fillText("Master Of Notes — teste", 48, 70);
    g.textAlign = "right";
    g.font = "32px sans-serif";
    g.fillText(`${clock(t)} / ${clock(songMs)}`, W - 48, 70);
    g.fillStyle = "#33333f";
    g.fillRect(48, 96, W - 96, 10);
    g.fillStyle = "#ff4f8b";
    g.fillRect(48, 96, (W - 96) * Math.min(1, t / songMs), 10);

    g.textAlign = "center";
    g.fillStyle = cur ? "#ffe066" : "#8a8a99";
    g.font = "bold 150px sans-serif";
    const label = cur ? noteName(cur.midi) : t < notes[0].startMs ? "Introdução" : next ? "Instrumental" : "Fim";
    g.fillText(label, W / 2, 330);
    g.font = "36px sans-serif";
    g.fillStyle = "#8a8a99";
    g.fillText(guide ? "com melodia-guia" : "karaokê (sem melodia-guia)", W / 2, 390);
    if (next) g.fillText(`próxima: ${noteName(next.midi)} em ${((next.startMs - t) / 1000).toFixed(1)} s`, W / 2, 440);

    // Lower-centre band = lyrics, as in a real karaoke video (the app overlay must not cover it).
    const phrase = phrases.find((p) => t < p[p.length - 1].startMs + p[p.length - 1].durMs + 500) ?? [];
    g.font = "bold 52px sans-serif";
    const parts = phrase.map((n) => `${n.lyric ?? "·"} `);
    let x = W / 2 - parts.reduce((w, s) => w + g.measureText(s).width, 0) / 2;
    g.textAlign = "left";
    phrase.forEach((n, i) => {
      g.fillStyle = n === cur ? "#ff4f8b" : t >= n.startMs + n.durMs ? "#f4f4f8" : "#6a6a78";
      g.fillText(parts[i], x, 640);
      x += g.measureText(parts[i]).width;
    });
  };

  // Song time 0 = the rec.start() call (audio clock): the file captures from the call, while the
  // start event can arrive ~2 s later on a cold encoder (measured). The recorder only starts once the
  // canvas delivers frames and the audio track has a connected source, so both run first.
  let t0 = ac.currentTime;
  // setInterval, not requestAnimationFrame: rAF does not fire for a headless page without a compositor frame.
  const painter = setInterval(draw, 1000 / 30);
  const keepAlive = ac.createOscillator();
  const mute = ac.createGain();
  mute.gain.value = 0;
  keepAlive.connect(mute).connect(dest);
  keepAlive.start();
  // Warm-up recording, discarded: initialises the encoders so the real start is prompt.
  const warm = new MediaRecorder(stream, { mimeType: "video/webm;codecs=vp8,opus" });
  await new Promise((r) => ((warm.onstart = r), warm.start()));
  await new Promise((r) => ((warm.onstop = r), warm.stop()));

  const started = new Promise<void>((r) => (rec.onstart = () => r()));
  const stopped = new Promise<void>((r) => (rec.onstop = () => r()));
  t0 = ac.currentTime;
  rec.start(1000);
  const wallStart = performance.now();
  const at = (ms: number) => t0 + ms / 1000;

  const tone = (wave: PeriodicWave | null, freq: number, from: number, to: number, level: number, ramp: number) => {
    const o = ac.createOscillator();
    if (wave) o.setPeriodicWave(wave);
    o.frequency.value = freq;
    const v = ac.createGain();
    const [s, e] = [Math.max(at(from), ac.currentTime + 0.01), at(to)];
    v.gain.setValueAtTime(0, s);
    v.gain.linearRampToValueAtTime(level, s + ramp);
    v.gain.setValueAtTime(level, Math.max(s + ramp, e - ramp));
    v.gain.linearRampToValueAtTime(0, e);
    o.connect(v).connect(dest);
    o.start(s);
    o.stop(e + 0.01);
  };
  BARS.forEach((c, i) => {
    if (i * 2000 >= songMs) return;
    for (const m of CHORDS[c]) tone(null, hz(m), i * 2000, Math.min((i + 1) * 2000, songMs), 0.05, 0.05);
  });
  if (guide) {
    // Light harmonics so the guide is clearly audible on laptop speakers.
    const voice = ac.createPeriodicWave(new Float32Array([0, 0, 0, 0]), new Float32Array([0, 1, 0.3, 0.12]));
    for (const n of notes) tone(voice, hz(n.midi), n.startMs, n.startMs + n.durMs, 0.3, 0.012);
  }
  await started;

  await new Promise<void>((r) => {
    const poll = () => (ac.currentTime - t0 >= songMs / 1000 ? r() : setTimeout(poll, 20));
    poll();
  });
  rec.stop();
  await stopped;
  clearInterval(painter);
  const recordedMs = performance.now() - wallStart;
  await ac.close();

  // MediaRecorder WebM has no Duration: insert one into Segment/Info so players show length and can seek.
  const bytes = new Uint8Array(await new Blob(chunks).arrayBuffer());
  const INFO = [0x15, 0x49, 0xa9, 0x66];
  const pos = bytes.findIndex((_, i) => INFO.every((b, k) => bytes[i + k] === b));
  if (pos < 0) throw new Error("WebM sem elemento Info");
  const sizeAt = pos + 4;
  const len = Math.clz32(bytes[sizeAt]) - 23; // vint length from leading zeros of the first byte
  let size = bytes[sizeAt] & (0xff >> len);
  for (let k = 1; k < len; k++) size = size * 256 + bytes[sizeAt + k];
  const durationEl = new Uint8Array(11);
  durationEl.set([0x44, 0x89, 0x88]);
  new DataView(durationEl.buffer).setFloat64(3, songMs); // TimecodeScale 1 ms (Chromium default)
  const newSize = new Uint8Array(8);
  newSize[0] = 0x01;
  new DataView(newSize.buffer).setUint32(4, size + durationEl.length);
  const contentEnd = sizeAt + len + size;
  const fixed = new Blob([bytes.slice(0, sizeAt), newSize, bytes.slice(sizeAt + len, contentEnd), durationEl, bytes.slice(contentEnd)], {
    type: "video/webm",
  });

  const a = document.createElement("a");
  a.href = URL.createObjectURL(fixed);
  a.download = fileName;
  a.click();
  return { recordedMs, bytes: fixed.size };
}

async function save(page: Page, path: string, trigger: () => Promise<unknown>) {
  const [download, result] = await Promise.all([page.waitForEvent("download", { timeout: SONG_MS + 60_000 }), trigger()]);
  await download.saveAs(path);
  return result;
}

/** Renders both videos and the reference into `outDir` (real time: ~2 × SONG_MS). The page must allow
 * audio without a user gesture (`--autoplay-policy=no-user-gesture-required`). */
export async function generateTestSong(page: Page, outDir: string) {
  const notes = melody.notes as Note[];
  const report: Record<string, unknown> = {};
  for (const guide of [true, false]) {
    const fileName = guide ? FILES.guide : FILES.karaoke;
    report[fileName] = await save(page, `${outDir}/${fileName}`, () =>
      page.evaluate(renderSong, { notes, songMs: SONG_MS, guide, fileName }),
    );
  }
  await save(page, `${outDir}/${FILES.reference}`, () =>
    page.evaluate(
      ([json, name]) => {
        const a = document.createElement("a");
        a.href = URL.createObjectURL(new Blob([json], { type: "application/json" }));
        a.download = name;
        a.click();
      },
      [`${JSON.stringify(melody, null, 2)}\n`, FILES.reference] as const,
    ),
  );
  return report;
}
