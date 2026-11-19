// @vitest-environment node
import { describe, expect, it } from "vitest";
import type { LiveTick } from "../lib/ipc";
import { createLeakMonitor } from "./leak";

const mic = (voiced: boolean) => ({ rmsDbfs: -20, f0Hz: voiced ? 220 : null, confidence: 0.9, voiced, clipped: false });
const silentPart = (voiced: boolean): LiveTick => ({ mic: mic(voiced), feedback: null });
const notePart = (voiced: boolean): LiveTick => ({
  mic: mic(voiced),
  feedback: { tMs: 0, targetMidi: 60, sungMidi: 60, centsError: 0, label: "excellent", runningScore: 90 },
});

describe("createLeakMonitor", () => {
  it("needs a full window before deciding", () => {
    const push = createLeakMonitor(10, 0.2);
    for (let i = 0; i < 9; i++) expect(push(silentPart(true))).toBe(false);
    expect(push(silentPart(true))).toBe(true);
  });

  it("flags > threshold voiced frames outside notes, not at the threshold", () => {
    const push = createLeakMonitor(10, 0.2);
    let r = false;
    for (let i = 0; i < 10; i++) r = push(silentPart(i < 2)); // exactly 20%
    expect(r).toBe(false);
    // The two oldest frames were voiced: replacing them with voiced frames keeps 20%.
    expect(push(silentPart(true))).toBe(false);
    expect(push(silentPart(true))).toBe(false);
    expect(push(silentPart(true))).toBe(true); // replaces an unvoiced frame: 30%
  });

  it("ignores frames inside notes (the singer is supposed to sing there)", () => {
    const push = createLeakMonitor(10, 0.2);
    for (let i = 0; i < 10; i++) push(silentPart(false));
    for (let i = 0; i < 100; i++) expect(push(notePart(true))).toBe(false);
  });

  it("recovers when the leak stops", () => {
    const push = createLeakMonitor(10, 0.2);
    for (let i = 0; i < 10; i++) push(silentPart(true));
    let r = true;
    for (let i = 0; i < 10; i++) r = push(silentPart(false));
    expect(r).toBe(false);
  });
});
