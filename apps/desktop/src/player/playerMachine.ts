// SPEC-003 R1: idle → loading → ready → countdown → playing ⇄ paused → completed; any → error.
// Pure reducer. Invalid transitions return the SAME state object (callers can rely on identity).

export type PlayerState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; durationMs: number }
  | { status: "countdown"; durationMs: number; remaining: number }
  | { status: "playing"; durationMs: number }
  | { status: "paused"; durationMs: number }
  | { status: "completed"; durationMs: number }
  | { status: "error"; message: string };

export type PlayerEvent =
  | { type: "LOAD" } // a new source was set on <video>
  | { type: "LOADED"; durationMs: number } // loadedmetadata
  | { type: "START"; countdown: number } // seconds; 0 = play immediately (practice)
  | { type: "TICK" } // one countdown second elapsed
  | { type: "PAUSE" }
  | { type: "RESUME" }
  | { type: "ENDED" }
  | { type: "FAIL"; message: string }
  | { type: "RESET" };

export const initialPlayerState: PlayerState = { status: "idle" };

export function playerReducer(s: PlayerState, e: PlayerEvent): PlayerState {
  switch (e.type) {
    case "FAIL":
      return { status: "error", message: e.message };
    case "RESET":
      return s.status === "idle" ? s : initialPlayerState;
    case "LOAD":
      return s.status === "idle" || s.status === "completed" || s.status === "error" ? { status: "loading" } : s;
    case "LOADED":
      return s.status === "loading" ? { status: "ready", durationMs: e.durationMs } : s;
    case "START":
      if (s.status !== "ready" && s.status !== "completed") return s;
      return e.countdown > 0
        ? { status: "countdown", durationMs: s.durationMs, remaining: Math.ceil(e.countdown) }
        : { status: "playing", durationMs: s.durationMs };
    case "TICK":
      if (s.status !== "countdown") return s;
      return s.remaining > 1 ? { ...s, remaining: s.remaining - 1 } : { status: "playing", durationMs: s.durationMs };
    case "PAUSE":
      return s.status === "playing" ? { status: "paused", durationMs: s.durationMs } : s;
    case "RESUME":
      return s.status === "paused" ? { status: "playing", durationMs: s.durationMs } : s;
    case "ENDED":
      return s.status === "playing" || s.status === "paused" ? { status: "completed", durationMs: s.durationMs } : s;
  }
}
