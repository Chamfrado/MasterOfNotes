import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../lib/ipc";
import { LEAK_TEST_S, LeakTest, pickLeakSong } from "./LeakTest";

vi.mock("../lib/ipc", () => ({
  listSongs: vi.fn(),
  getReference: vi.fn(),
  startPerformance: vi.fn(),
  syncPerformance: vi.fn(() => Promise.resolve()),
  finishPerformance: vi.fn(),
  abortPerformance: vi.fn(() => Promise.resolve()),
  stopMicMonitor: vi.fn(() => Promise.resolve()),
  videoSrc: () => "asset://v.mp4",
}));

const song = (over: Partial<ipc.Song>) =>
  ({ id: "x", title: "x", available: true, playable: "yes", referenceStatus: "none", ...over }) as ipc.Song;
const settings = { inputDeviceId: null, videoVolume: 0.7, leakCheck: null } as ipc.AppSettings;

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers({ shouldAdvanceTime: true });
  const proto = HTMLMediaElement.prototype as unknown as Record<string, unknown>;
  proto.play = () => Promise.resolve();
  proto.pause = () => {};
  proto.requestVideoFrameCallback = () => 1;
  proto.cancelVideoFrameCallback = () => {};
  Object.defineProperty(HTMLMediaElement.prototype, "readyState", { value: 4, configurable: true });
});
afterEach(() => vi.useRealTimers());

describe("pickLeakSong", () => {
  it("prefers validated, then experimental, skipping unplayable/unavailable", () => {
    const exp = song({ id: "e", referenceStatus: "experimental" });
    const val = song({ id: "v", referenceStatus: "validated" });
    expect(pickLeakSong([exp, val])?.id).toBe("v");
    expect(pickLeakSong([exp, song({ id: "bad", referenceStatus: "validated", playable: "no" })])?.id).toBe("e");
    expect(pickLeakSong([song({ referenceStatus: "validated", available: false }), song({})])).toBeNull();
  });
});

describe("LeakTest", () => {
  it("explains how to enable the test when no song has a reference", async () => {
    vi.mocked(ipc.listSongs).mockResolvedValue([song({})]);
    render(<LeakTest settings={settings} onFinished={vi.fn()} />);
    expect(await screen.findByText(/Importe uma referência/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Fazer teste" })).toBeDisabled();
  });

  it("plays from the first note silently, then reports a pass", async () => {
    vi.mocked(ipc.listSongs).mockResolvedValue([song({ id: "v", title: "Aquarela", referenceStatus: "validated" })]);
    vi.mocked(ipc.getReference).mockResolvedValue({
      id: 1,
      songId: "v",
      source: "json",
      status: "validated",
      revision: 1,
      offsetMs: 500,
      notes: [{ startMs: 12_000, durMs: 400, midi: 60, lyric: null }],
    });
    vi.mocked(ipc.startPerformance).mockResolvedValue({ competitive: false, reason: null });
    vi.mocked(ipc.finishPerformance).mockResolvedValue({
      performanceId: null,
      competitive: false,
      result: null,
      exploratory: null,
      leak: { passed: true, guideScore: 3, reason: null },
      personalBest: null,
      songRecord: null,
    });
    const onFinished = vi.fn();
    const { container } = render(<LeakTest settings={settings} onFinished={onFinished} />);
    expect(await screen.findByText("Música do teste: Aquarela")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Fazer teste" }));
    expect(await screen.findByText(`Silêncio… ${LEAK_TEST_S} s`)).toBeInTheDocument();

    const video = container.querySelector("video")!;
    expect(video.currentTime).toBe(12.5);
    expect(video.volume).toBe(0.7);
    expect(ipc.stopMicMonitor).toHaveBeenCalled();
    expect(ipc.startPerformance).toHaveBeenCalledWith(
      { songId: "v", roundId: null, participantId: null, leakCheck: true },
      expect.any(Function),
    );

    for (let i = 0; i < LEAK_TEST_S; i++) await act(() => vi.advanceTimersByTimeAsync(1000));
    expect(await screen.findByText(/Aprovado: partidas/)).toBeInTheDocument();
    expect(ipc.syncPerformance).toHaveBeenLastCalledWith(12.5, false, false);
    expect(ipc.abortPerformance).not.toHaveBeenCalled();
    expect(onFinished).toHaveBeenCalled();
  });

  it("shows a previous failed check and surfaces run errors", async () => {
    vi.mocked(ipc.listSongs).mockResolvedValue([song({ id: "v", referenceStatus: "experimental" })]);
    vi.mocked(ipc.getReference).mockResolvedValue({
      id: 1,
      songId: "v",
      source: "json",
      status: "experimental",
      revision: 1,
      offsetMs: 0,
      notes: [{ startMs: 0, durMs: 400, midi: 60, lyric: null }],
    });
    vi.mocked(ipc.startPerformance).mockResolvedValue({ competitive: false, reason: null });
    vi.mocked(ipc.finishPerformance).mockRejectedValue({ code: "audio_device", message: "Microfone desconectado." });
    const onFinished = vi.fn();
    render(<LeakTest settings={{ ...settings, leakCheck: { deviceId: null, videoVolume: 0.7, passed: false, atMs: 1 } }} onFinished={onFinished} />);
    expect(await screen.findByText("Reprovado")).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Testar de novo" }));
    for (let i = 0; i < LEAK_TEST_S + 1; i++) await act(() => vi.advanceTimersByTimeAsync(1000));
    expect(await screen.findByText("Microfone desconectado.")).toBeInTheDocument();
    expect(onFinished).toHaveBeenCalled();
  });
});
