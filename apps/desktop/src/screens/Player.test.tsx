import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components/ui";
import * as ipc from "../lib/ipc";
import { NavContext } from "../nav";
import { Player, clock } from "./Player";

vi.mock("../lib/ipc", () => ({
  listSongs: vi.fn(),
  getSettings: vi.fn(),
  startPerformance: vi.fn(),
  syncPerformance: vi.fn(() => Promise.resolve()),
  finishPerformance: vi.fn(),
  abortPerformance: vi.fn(() => Promise.resolve()),
  updateSong: vi.fn(() => Promise.resolve()),
  updateSettings: vi.fn(() => Promise.resolve()),
  videoSrc: () => "asset://song.mp4",
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ setFullscreen: vi.fn(() => Promise.resolve()), isFullscreen: () => Promise.resolve(false) }),
}));

const song: ipc.Song = {
  id: "s1",
  path: "C:/k/a.mp4",
  fileName: "a.mp4",
  title: "Aquarela",
  artist: null,
  extension: "mp4",
  sizeBytes: 1,
  durationMs: null,
  playable: "unknown",
  available: true,
  referenceStatus: "validated",
};
const settings = { videoVolume: 0.8, feedbackOverlay: "full" } as ipc.AppSettings;
const navigate = vi.fn();

// jsdom has no media pipeline: emulate play/pause state + events, and the frame callback.
beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers({ shouldAdvanceTime: true });
  vi.mocked(ipc.listSongs).mockResolvedValue([song]);
  vi.mocked(ipc.getSettings).mockResolvedValue(settings);
  vi.mocked(ipc.startPerformance).mockResolvedValue({ competitive: true, reason: null });
  const proto = HTMLMediaElement.prototype as unknown as Record<string, unknown>;
  proto.play = function (this: HTMLMediaElement) {
    Object.defineProperty(this, "paused", { value: false, configurable: true });
    this.dispatchEvent(new Event("play"));
    return Promise.resolve();
  };
  proto.pause = function (this: HTMLMediaElement) {
    Object.defineProperty(this, "paused", { value: true, configurable: true });
    this.dispatchEvent(new Event("pause"));
  };
  proto.requestVideoFrameCallback = () => 1;
  proto.cancelVideoFrameCallback = () => {};
});
afterEach(() => vi.useRealTimers());

async function renderToPlaying() {
  const { container, unmount } = render(
    <NavContext.Provider value={{ route: { name: "player", songId: "s1" }, navigate, back: vi.fn() }}>
      <ToastProvider>
        <Player songId="s1" />
      </ToastProvider>
    </NavContext.Provider>,
  );
  await screen.findByText("Carregando vídeo…");
  const video = container.querySelector("video")!;
  Object.defineProperty(video, "duration", { value: 120, configurable: true });
  Object.defineProperty(video, "paused", { value: true, configurable: true });
  fireEvent.loadedMetadata(video);
  fireEvent.click(await screen.findByRole("button", { name: "Começar" }));
  expect(await screen.findByText("3")).toBeInTheDocument();
  for (let i = 0; i < 3; i++) await act(() => vi.advanceTimersByTimeAsync(1000));
  return Object.assign(video, { unmountPlayer: unmount });
}

describe("clock", () => {
  it("formats seconds", () => {
    expect(clock(0)).toBe("0:00");
    expect(clock(185.9)).toBe("3:05");
    expect(clock(Number.NaN)).toBe("0:00");
  });
});

describe("Player", () => {
  it("loads metadata, counts down, plays and syncs with the real IPC calls", async () => {
    const video = await renderToPlaying();
    expect(ipc.updateSong).toHaveBeenCalledWith("s1", { durationMs: 120_000, playable: "yes" });
    expect(ipc.startPerformance).toHaveBeenCalledWith({ songId: "s1", roundId: null, participantId: null }, expect.any(Function));
    expect(video.paused).toBe(false);
    expect(ipc.syncPerformance).toHaveBeenCalledWith(0, true, false);
    expect(await screen.findByRole("button", { name: "Pausar" })).toBeInTheDocument();
    expect(video.volume).toBe(0.8);
  });

  it("Space pauses and resumes", async () => {
    const video = await renderToPlaying();
    fireEvent.keyDown(window, { key: " " });
    expect(video.paused).toBe(true);
    expect(ipc.syncPerformance).toHaveBeenLastCalledWith(0, false, false);
    expect(await screen.findByRole("button", { name: "Continuar" })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: " " });
    expect(video.paused).toBe(false);
  });

  it("asks before a seek turns a competitive run into practice", async () => {
    const video = await renderToPlaying();
    fireEvent.keyDown(window, { key: "ArrowRight" });
    expect(screen.getByRole("dialog", { name: "Pular trecho?" })).toBeInTheDocument();
    expect(video.currentTime).toBe(0);
    fireEvent.click(screen.getByRole("button", { name: "Pular e virar treino" }));
    expect(video.currentTime).toBe(5);
    expect(screen.getAllByText(/Treino/).length).toBeGreaterThan(0);
    // Already practice: further seeks need no confirmation.
    fireEvent.keyDown(window, { key: "ArrowRight" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(video.currentTime).toBe(10);
  });

  it("volume keys and H persist settings", async () => {
    const video = await renderToPlaying();
    fireEvent.keyDown(window, { key: "ArrowUp" });
    expect(video.volume).toBe(0.85);
    expect(ipc.updateSettings).toHaveBeenCalledWith({ videoVolume: 0.85 });
    fireEvent.keyDown(window, { key: "h" });
    expect(ipc.updateSettings).toHaveBeenCalledWith({ feedbackOverlay: "compact" });
    expect(screen.getByRole("button", { name: /Painel: compacto/ })).toBeInTheDocument();
  });

  it("finishes the performance and opens the result on ended", async () => {
    const outcome: ipc.PerformanceOutcome = {
      performanceId: 7,
      competitive: true,
      result: null,
      exploratory: null,
      personalBest: null,
      songRecord: null,
    };
    vi.mocked(ipc.finishPerformance).mockResolvedValue(outcome);
    const video = await renderToPlaying();
    fireEvent.ended(video);
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(navigate).toHaveBeenCalledWith({ name: "result", songId: "s1", outcome });
  });

  it("warns when the mic picks up the music outside notes (R9)", async () => {
    await renderToPlaying();
    const onTick = vi.mocked(ipc.startPerformance).mock.calls[0][1];
    const tick: ipc.LiveTick = { mic: { rmsDbfs: -20, f0Hz: 220, confidence: 0.9, voiced: true, clipped: false }, feedback: null };
    act(() => {
      for (let i = 0; i < 300; i++) onTick(tick);
    });
    expect(screen.getByText(/captando a música/)).toBeInTheDocument();
  });

  it("shows the codec hint and marks the song unplayable on media error", async () => {
    const { container } = render(
      <NavContext.Provider value={{ route: { name: "player", songId: "s1" }, navigate, back: vi.fn() }}>
        <ToastProvider>
          <Player songId="s1" />
        </ToastProvider>
      </NavContext.Provider>,
    );
    await screen.findByText("Carregando vídeo…");
    fireEvent.error(container.querySelector("video")!);
    expect(screen.getByText(/H\.264 e áudio AAC/)).toBeInTheDocument();
    expect(ipc.updateSong).toHaveBeenCalledWith("s1", { playable: "no" });
  });

  it("aborts an open performance when leaving", async () => {
    const video = await renderToPlaying();
    video.unmountPlayer();
    expect(ipc.abortPerformance).toHaveBeenCalledTimes(1);
  });
});
