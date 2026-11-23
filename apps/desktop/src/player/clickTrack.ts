// T-004-07: click track for the latency probe, played through a media element (same output path as videos).

const CLICK_FIRST_MS = 1000; // ≥ 500 ms so playback has settled before the first click
const CLICK_PERIOD_MS = 750;
export const CLICK_COUNT = 8;
const TAIL_MS = 1000;
const CLICK_LEN_MS = 6;
const CLICK_HZ = 2000;

/** 16-bit mono PCM WAV with short decaying 2 kHz clicks; returns the bytes and the click times (media time, ms). */
export function clickTrack(sampleRate = 48_000) {
  const clicksMs = Array.from({ length: CLICK_COUNT }, (_, i) => CLICK_FIRST_MS + i * CLICK_PERIOD_MS);
  const totalMs = clicksMs[CLICK_COUNT - 1] + TAIL_MS;
  const n = Math.ceil((totalMs / 1000) * sampleRate);
  const buf = new ArrayBuffer(44 + n * 2);
  const v = new DataView(buf);
  const ascii = (at: number, s: string) => [...s].forEach((c, i) => v.setUint8(at + i, c.charCodeAt(0)));
  ascii(0, "RIFF");
  v.setUint32(4, 36 + n * 2, true);
  ascii(8, "WAVE");
  ascii(12, "fmt ");
  v.setUint32(16, 16, true); // PCM chunk size
  v.setUint16(20, 1, true); // PCM
  v.setUint16(22, 1, true); // mono
  v.setUint32(24, sampleRate, true);
  v.setUint32(28, sampleRate * 2, true); // byte rate
  v.setUint16(32, 2, true); // block align
  v.setUint16(34, 16, true); // bits
  ascii(36, "data");
  v.setUint32(40, n * 2, true);
  const len = Math.round((CLICK_LEN_MS / 1000) * sampleRate);
  for (const ms of clicksMs) {
    const start = Math.round((ms / 1000) * sampleRate); // sample-exact onset
    for (let k = 0; k < len && start + k < n; k++) {
      const s = 0.9 * Math.exp(-k / (len / 5)) * Math.sin((2 * Math.PI * CLICK_HZ * k) / sampleRate + Math.PI / 2);
      v.setInt16(44 + (start + k) * 2, Math.round(s * 32767), true);
    }
  }
  return { wav: new Uint8Array(buf), clicksMs, durationMs: totalMs };
}
