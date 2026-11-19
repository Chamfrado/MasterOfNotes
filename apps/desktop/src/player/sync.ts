// R6 (SPEC-003): report the video's own media clock to Rust every ~250 ms while it plays.
import { errorMessage } from "../components/ui";
import { syncPerformance } from "../lib/ipc";

export const SYNC_EVERY_MS = 250;
export const warn = (what: string) => (e: unknown) => console.warn(`${what}:`, errorMessage(e));

/** Starts the requestVideoFrameCallback loop (metadata.mediaTime, never the UI clock). Returns stop(). */
export function startVideoSync(v: HTMLVideoElement): () => void {
  let handle = 0;
  let last = 0;
  const onFrame: VideoFrameRequestCallback = (now, meta) => {
    if (now - last >= SYNC_EVERY_MS) {
      last = now;
      syncPerformance(meta.mediaTime, true, false).catch(warn("sync_performance"));
    }
    handle = v.requestVideoFrameCallback(onFrame);
  };
  handle = v.requestVideoFrameCallback(onFrame);
  return () => v.cancelVideoFrameCallback(handle);
}
