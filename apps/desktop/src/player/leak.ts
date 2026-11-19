// SPEC-004 R9: "the mic is picking up the music". The UI only knows the singer should be silent
// where there is no target note (LiveTick.feedback === null), so that is where we measure.
import type { AppSettings, LiveTick } from "../lib/ipc";

export const LEAK_WINDOW = 300; // ~3 s of 10 ms frames outside notes
export const LEAK_THRESHOLD = 0.2; // > 20% voiced frames while silent → leak

/** Returns push(tick) → true while the recent silent-section frames look like leakage. */
export function createLeakMonitor(window = LEAK_WINDOW, threshold = LEAK_THRESHOLD) {
  const ring = new Uint8Array(window);
  let filled = 0;
  let next = 0;
  let voiced = 0;
  return (tick: LiveTick): boolean => {
    if (tick.feedback === null) {
      if (filled === window) voiced -= ring[next];
      else filled++;
      ring[next] = tick.mic.voiced ? 1 : 0;
      voiced += ring[next];
      next = (next + 1) % window;
    }
    return filled === window && voiced / window > threshold;
  };
}

/** Same slack as the Rust gate (live.rs): volume ≤ checked volume + 0.05. */
export const LEAK_VOLUME_TOLERANCE = 0.05 + 1e-9;

export type LeakCheckState = "passed" | "failed" | "stale" | "none";
/**
 * T-004-06: competitive play needs a passed leak check for the current device at <= the checked volume.
 * Mirrors the rule Rust enforces, so the UI can explain it before the run starts.
 */
export function leakCheckState(s: Pick<AppSettings, "leakCheck" | "inputDeviceId" | "videoVolume">): LeakCheckState {
  const c = s.leakCheck;
  if (!c) return "none";
  if (c.deviceId !== s.inputDeviceId || s.videoVolume > c.videoVolume + LEAK_VOLUME_TOLERANCE) return "stale";
  return c.passed ? "passed" : "failed";
}
