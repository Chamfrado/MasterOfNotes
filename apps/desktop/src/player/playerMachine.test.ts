// @vitest-environment node
import { describe, expect, it } from "vitest";
import { initialPlayerState, playerReducer, type PlayerEvent, type PlayerState } from "./playerMachine";

const D = 180_000;
const states: Record<PlayerState["status"], PlayerState> = {
  idle: { status: "idle" },
  loading: { status: "loading" },
  ready: { status: "ready", durationMs: D },
  countdown: { status: "countdown", durationMs: D, remaining: 2 },
  playing: { status: "playing", durationMs: D },
  paused: { status: "paused", durationMs: D },
  completed: { status: "completed", durationMs: D },
  error: { status: "error", message: "Codec não suportado" },
};
const events: Record<PlayerEvent["type"], PlayerEvent> = {
  LOAD: { type: "LOAD" },
  LOADED: { type: "LOADED", durationMs: D },
  START: { type: "START", countdown: 3 },
  TICK: { type: "TICK" },
  PAUSE: { type: "PAUSE" },
  RESUME: { type: "RESUME" },
  ENDED: { type: "ENDED" },
  FAIL: { type: "FAIL", message: "boom" },
};

// Every valid transition, as "from:EVENT" → resulting state. Anything not listed must be a no-op.
const valid: Record<string, PlayerState> = {
  "idle:LOAD": { status: "loading" },
  "completed:LOAD": { status: "loading" },
  "error:LOAD": { status: "loading" },
  "loading:LOADED": { status: "ready", durationMs: D },
  "ready:START": { status: "countdown", durationMs: D, remaining: 3 },
  "completed:START": { status: "countdown", durationMs: D, remaining: 3 },
  "countdown:TICK": { status: "countdown", durationMs: D, remaining: 1 },
  "playing:PAUSE": { status: "paused", durationMs: D },
  "paused:RESUME": { status: "playing", durationMs: D },
  "playing:ENDED": { status: "completed", durationMs: D },
  "paused:ENDED": { status: "completed", durationMs: D },
};
for (const s of Object.keys(states)) {
  valid[`${s}:FAIL`] = { status: "error", message: "boom" };
}

describe("playerReducer — full transition table", () => {
  for (const [sName, s] of Object.entries(states)) {
    for (const [eName, e] of Object.entries(events)) {
      const key = `${sName}:${eName}`;
      if (key in valid) {
        it(`${key} → ${valid[key].status}`, () => {
          expect(playerReducer(s, e)).toEqual(valid[key]);
        });
      } else {
        it(`${key} is ignored (same object)`, () => {
          expect(playerReducer(s, e)).toBe(s);
        });
      }
    }
  }
});

describe("playerReducer — countdown and happy path", () => {
  it("counts 3-2-1 then plays", () => {
    let s: PlayerState = { status: "ready", durationMs: D };
    s = playerReducer(s, { type: "START", countdown: 3 });
    expect(s).toEqual({ status: "countdown", durationMs: D, remaining: 3 });
    s = playerReducer(s, { type: "TICK" });
    s = playerReducer(s, { type: "TICK" });
    expect(s).toEqual({ status: "countdown", durationMs: D, remaining: 1 });
    s = playerReducer(s, { type: "TICK" });
    expect(s).toEqual({ status: "playing", durationMs: D });
  });

  it("START with countdown 0 plays immediately (practice)", () => {
    expect(playerReducer(states.ready, { type: "START", countdown: 0 })).toEqual({ status: "playing", durationMs: D });
  });

  it("runs the whole vertical flow", () => {
    const flow: PlayerEvent[] = [
      events.LOAD,
      events.LOADED,
      { type: "START", countdown: 1 },
      events.TICK,
      events.PAUSE,
      events.RESUME,
      events.ENDED,
    ];
    expect(flow.reduce(playerReducer, initialPlayerState)).toEqual({ status: "completed", durationMs: D });
  });
});
