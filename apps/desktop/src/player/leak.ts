// SPEC-004 R9: "the mic is picking up the music". The UI only knows the singer should be silent
// where there is no target note (LiveTick.feedback === null), so that is where we measure.
import type { LiveTick } from "../lib/ipc";

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
