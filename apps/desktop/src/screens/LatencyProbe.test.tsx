import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../lib/ipc";
import { CLICK_COUNT } from "../player/clickTrack";
import { LatencyProbe } from "./LatencyProbe";

vi.mock("../lib/ipc", () => ({
  listSongs: vi.fn(),
  startPerformance: vi.fn(),
  syncPerformance: vi.fn(() => Promise.resolve()),
  finishPerformance: vi.fn(),
  abortPerformance: vi.fn(() => Promise.resolve()),
  stopMicMonitor: vi.fn(() => Promise.resolve()),
}));

const outcome = (latency: ipc.LatencyVerdict | null): ipc.PerformanceOutcome => ({
  performanceId: null,
  competitive: false,
  result: null,
  exploratory: null,
  latency,
  personalBest: null,
  songRecord: null,
});

beforeEach(() => {
  vi.clearAllMocks();
  URL.createObjectURL = vi.fn(() => "blob:clicks");
  URL.revokeObjectURL = vi.fn();
  (HTMLMediaElement.prototype as unknown as Record<string, unknown>).play = function (this: HTMLMediaElement) {
    this.dispatchEvent(new Event("playing"));
    return Promise.resolve();
  };
  (HTMLMediaElement.prototype as unknown as Record<string, unknown>).pause = () => {};
  vi.mocked(ipc.listSongs).mockResolvedValue([{ id: "s1", available: true } as ipc.Song]);
  vi.mocked(ipc.startPerformance).mockResolvedValue({ competitive: false, reason: null });
});

async function runProbe() {
  const onFinished = vi.fn();
  const { container } = render(<LatencyProbe onFinished={onFinished} />);
  fireEvent.click(screen.getByRole("button", { name: "Medir latência" }));
  const audio = container.querySelector("audio")!;
  await vi.waitFor(() => expect(ipc.startPerformance).toHaveBeenCalled());
  await act(async () => {
    audio.dispatchEvent(new Event("ended"));
  });
  return { onFinished, audio };
}

describe("LatencyProbe", () => {
  it("plays the click track, sends click times and shows the saved offset", async () => {
    vi.mocked(ipc.finishPerformance).mockResolvedValue(
      outcome({ offsetMs: 142.4, jitterMs: 8.6, detected: 7, expected: 8, passed: true, reason: null }),
    );
    const { onFinished, audio } = await runProbe();
    const [args] = vi.mocked(ipc.startPerformance).mock.calls[0];
    expect(args).toMatchObject({ songId: "s1", roundId: null, participantId: null });
    expect(args.latencyProbe).toHaveLength(CLICK_COUNT);
    expect(args.latencyProbe![0]).toBeGreaterThanOrEqual(500);
    expect(audio.src).toBe("blob:clicks");
    expect(ipc.stopMicMonitor).toHaveBeenCalled();
    expect(ipc.syncPerformance).toHaveBeenCalledWith(0, true, false);
    expect(ipc.syncPerformance).toHaveBeenLastCalledWith(0, false, false);
    expect(await screen.findByText(/Latência medida: 142 ms \(variação ±9 ms, 7 de 8 palmas\)/)).toBeInTheDocument();
    expect(onFinished).toHaveBeenCalled();
    expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:clicks");
  });

  it("shows Rust's reason when the probe fails", async () => {
    vi.mocked(ipc.finishPerformance).mockResolvedValue(
      outcome({ offsetMs: 0, jitterMs: 0, detected: 1, expected: 8, passed: false, reason: "Poucas palmas detectadas." }),
    );
    await runProbe();
    expect(await screen.findByText(/Poucas palmas detectadas\. O ajuste atual foi mantido\./)).toBeInTheDocument();
  });

  it("needs a song in the library (start_performance requires one)", async () => {
    vi.mocked(ipc.listSongs).mockResolvedValue([]);
    render(<LatencyProbe onFinished={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Medir latência" }));
    expect(await screen.findByText(/Adicione ao menos uma música/)).toBeInTheDocument();
    expect(ipc.startPerformance).not.toHaveBeenCalled();
    expect(ipc.abortPerformance).not.toHaveBeenCalled();
  });

  it("surfaces errors from finishing the run", async () => {
    vi.mocked(ipc.finishPerformance).mockRejectedValue({ code: "audio_device", message: "Microfone desconectado." });
    await runProbe();
    expect(await screen.findByText("Microfone desconectado.")).toBeInTheDocument();
  });
});
