// @vitest-environment node
import { describe, expect, it } from "vitest";
import { CLICK_COUNT, clickTrack } from "./clickTrack";

describe("clickTrack", () => {
  const sr = 48_000;
  const { wav, clicksMs, durationMs } = clickTrack(sr);
  const v = new DataView(wav.buffer);
  const sample = (i: number) => v.getInt16(44 + i * 2, true);
  const text = (at: number, len: number) => String.fromCharCode(...wav.slice(at, at + len));

  it("is 8 clicks, 750 ms apart, first at ≥ 500 ms", () => {
    expect(clicksMs).toHaveLength(CLICK_COUNT);
    expect(clicksMs[0]).toBeGreaterThanOrEqual(500);
    expect(clicksMs.slice(1).map((t, i) => t - clicksMs[i])).toEqual(Array(CLICK_COUNT - 1).fill(750));
  });

  it("is a valid 16-bit mono PCM WAV of the right length", () => {
    expect(text(0, 4)).toBe("RIFF");
    expect(text(8, 4)).toBe("WAVE");
    expect(v.getUint16(20, true)).toBe(1);
    expect(v.getUint16(22, true)).toBe(1);
    expect(v.getUint32(24, true)).toBe(sr);
    expect(v.getUint16(34, true)).toBe(16);
    const samples = v.getUint32(40, true) / 2;
    expect(samples).toBe(Math.ceil((durationMs / 1000) * sr));
    expect(wav.byteLength).toBe(44 + samples * 2);
  });

  it("has sample-exact onsets: silence before each click, loud at the click", () => {
    for (const ms of clicksMs) {
      const at = Math.round((ms / 1000) * sr);
      expect(sample(at - 1)).toBe(0);
      expect(Math.abs(sample(at))).toBeGreaterThan(0.8 * 32767);
    }
    expect(sample(Math.round(0.5 * sr))).toBe(0); // silence between start and first click
  });
});
