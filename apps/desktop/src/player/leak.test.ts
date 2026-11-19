// @vitest-environment node
import { describe, expect, it } from "vitest";
import type { LiveTick } from "../lib/ipc";
import { createLeakMonitor, leakCheckState } from "./leak";

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

describe("leakCheckState", () => {
  const check = { deviceId: "usb", videoVolume: 0.6, passed: true, atMs: 1 };
  it("needs a check", () => {
    expect(leakCheckState({ leakCheck: null, inputDeviceId: "usb", videoVolume: 0.5 })).toBe("none");
  });
  it("passes on the same device at or below the checked volume", () => {
    expect(leakCheckState({ leakCheck: check, inputDeviceId: "usb", videoVolume: 0.6 })).toBe("passed");
    expect(leakCheckState({ leakCheck: check, inputDeviceId: "usb", videoVolume: 0.3 })).toBe("passed");
  });
  it("is stale on another device or a louder volume", () => {
    expect(leakCheckState({ leakCheck: check, inputDeviceId: null, videoVolume: 0.6 })).toBe("stale");
    expect(leakCheckState({ leakCheck: check, inputDeviceId: "usb", videoVolume: 0.65 })).toBe("passed"); // Rust slack +0.05
    expect(leakCheckState({ leakCheck: check, inputDeviceId: "usb", videoVolume: 0.7 })).toBe("stale");
  });
  it("reports a failed check", () => {
    expect(leakCheckState({ leakCheck: { ...check, passed: false }, inputDeviceId: "usb", videoVolume: 0.6 })).toBe("failed");
  });
});
