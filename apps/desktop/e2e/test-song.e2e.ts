// T-003-05: the generated test song plays in Chromium, has a real duration, seeks, and its audio matches
// the exported reference (every note's pitch present in the guide version, absent in the karaoke version,
// onsets aligned). The reference JSON itself is parsed by the real parser in crates/game/tests/test_song.rs.
import { expect, test } from "@playwright/test";
import { FILES, MELODY_JSON, SONG_MS, generateTestSong } from "./tools/test-song.ts";

test.use({ launchOptions: { args: ["--autoplay-policy=no-user-gesture-required"] } });

test("música de teste: toca, tem duração, permite seek e o áudio bate com a referência", async ({ page }, info) => {
  test.setTimeout(SONG_MS * 2 + 120_000); // rendering is real time
  const dir = info.outputPath("song");
  console.log(await generateTestSong(page, dir));

  // Serve the files through the Vite dev server (HTTP ranges, same as the app's asset protocol).
  const base = `/${dir.replace(/\\/g, "/").split("/apps/desktop/")[1].split("/").map(encodeURIComponent).join("/")}`;
  await page.goto(`${base}/${FILES.reference}`);
  expect(await page.evaluate(() => JSON.parse(document.body.innerText))).toEqual(MELODY_JSON);

  const media = (url: string) =>
    page.evaluate(async ({ url, songMs }) => {
      const v = document.createElement("video");
      v.muted = true;
      v.src = url;
      document.body.append(v);
      await new Promise((r, j) => ((v.onloadedmetadata = r), (v.onerror = () => j(new Error(`erro ${v.error?.code}`)))));
      const duration = v.duration;
      await v.play();
      await new Promise((r) => setTimeout(r, 1000));
      const played = v.currentTime;
      const frames = v.getVideoPlaybackQuality().totalVideoFrames;
      v.pause();
      v.currentTime = songMs / 1000 - 10;
      await new Promise((r) => (v.onseeked = r));
      // The frame shown after the seek must be the one for that time: the progress bar (x 48..1232, y 101)
      // is pink just before the seek position and grey just after it.
      const c = document.createElement("canvas");
      [c.width, c.height] = [v.videoWidth, v.videoHeight];
      const g = c.getContext("2d");
      g?.drawImage(v, 0, 0);
      const bar = 48 + 1184 * (v.currentTime / (songMs / 1000));
      const red = (x: number) => g?.getImageData(Math.round(x), 101, 1, 1).data[0] ?? 0;
      return { duration, played, frames, seekedTo: v.currentTime, width: v.videoWidth, barBefore: red(bar - 25), barAfter: red(bar + 25) };
    }, { url, songMs: SONG_MS });

  const notes = MELODY_JSON.notes;
  /** Goertzel power per note: target pitch and ±1 semitone, over the central part of the note, plus onset error. */
  const analyze = (url: string) =>
    page.evaluate(async ({ url, notes }) => {
      const ac = new AudioContext({ sampleRate: 48_000 });
      const buf = await ac.decodeAudioData(await (await fetch(url)).arrayBuffer());
      const x = buf.getChannelData(0);
      const sr = buf.sampleRate;
      const power = (f: number, fromMs: number, ms: number) => {
        const [s, n] = [Math.round((fromMs / 1000) * sr), Math.round((ms / 1000) * sr)];
        const w = (2 * Math.PI * f) / sr;
        let [s1, s2] = [0, 0];
        for (let i = s; i < s + n && i < x.length; i++) [s1, s2] = [x[i] + 2 * Math.cos(w) * s1 - s2, s1];
        return (s1 * s1 + s2 * s2 - 2 * Math.cos(w) * s1 * s2) / (n * n);
      };
      const hz = (m: number) => 440 * 2 ** ((m - 69) / 12);
      return notes.map((n) => {
        const f = hz(n.midi);
        const len = Math.min(200, n.durMs * 0.6);
        const from = n.startMs + (n.durMs - len) / 2;
        const target = power(f, from, len);
        const neighbour = Math.max(power(f * 2 ** (1 / 12), from, len), power(f * 2 ** (-1 / 12), from, len));
        // Onset: first 10 ms window (1 ms steps) reaching half the steady power, reported at the window centre.
        let onset = NaN;
        for (let t = n.startMs - 100; t < n.startMs + 100; t++) {
          if (power(f, t - 5, 10) >= target / 2) {
            onset = t - n.startMs;
            break;
          }
        }
        return { startMs: n.startMs, target, ratio: target / neighbour, onset };
      });
    }, { url, notes });

  for (const file of [FILES.guide, FILES.karaoke]) {
    const m = await media(`${base}/${file}`);
    console.log(file, m);
    expect(Math.abs(m.duration - SONG_MS / 1000)).toBeLessThan(0.3);
    expect(m.played).toBeGreaterThan(0.5);
    expect(m.frames).toBeGreaterThan(0);
    expect(Math.abs(m.seekedTo - (SONG_MS / 1000 - 10))).toBeLessThan(0.5);
    expect(m.width).toBe(1280);
    expect(m.barBefore, "após o seek, o quadro exibido é o do tempo pedido (barra rosa antes)").toBeGreaterThan(200);
    expect(m.barAfter, "após o seek, o quadro exibido é o do tempo pedido (barra cinza depois)").toBeLessThan(100);
  }

  const guide = await analyze(`${base}/${FILES.guide}`);
  const karaoke = await analyze(`${base}/${FILES.karaoke}`);
  const table = guide.map((g, i) => ({
    startMs: g.startMs,
    ratio: Math.round(g.ratio),
    karaokeLeak: +(karaoke[i].target / g.target).toFixed(5),
    onsetMs: g.onset,
  }));
  console.table(table);
  for (const row of table) {
    expect(row.ratio, `nota em ${row.startMs} ms: altura da referência dominante`).toBeGreaterThan(10);
    expect(row.karaokeLeak, `nota em ${row.startMs} ms: karaokê sem melodia-guia`).toBeLessThan(0.01);
  }
  // Notes entered after a rest must start within ±30 ms of the reference. (Legato entries are not measured:
  // a 10 ms Goertzel window is ~100 Hz wide and still hears the previous note.)
  const entries = table.filter((_, i) => i === 0 || notes[i].startMs - (notes[i - 1].startMs + notes[i - 1].durMs) >= 100);
  expect(entries.length).toBeGreaterThanOrEqual(5);
  for (const row of entries) expect(Math.abs(row.onsetMs), `onset da nota em ${row.startMs} ms`).toBeLessThanOrEqual(30);
});
