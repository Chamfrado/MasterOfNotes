import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components/ui";
import * as ipc from "../lib/ipc";
import { NavContext } from "../nav";
import { Reference, rollLayout } from "./Reference";

const replace = vi.fn();
vi.mock("../lib/ipc", () => ({
  listSongs: vi.fn(),
  getReference: vi.fn(),
  listMidiTracks: vi.fn(),
  importReference: vi.fn(),
  setReferenceOffset: vi.fn(),
  setReferenceStatus: vi.fn(),
  startPerformance: vi.fn(),
  syncPerformance: vi.fn(),
  finishPerformance: vi.fn(),
  abortPerformance: vi.fn(),
  videoSrc: () => "asset://v.mp4",
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
import { open } from "@tauri-apps/plugin-dialog";

const song = { id: "s1", title: "Aquarela", available: true } as ipc.Song;
const ref = (over: Partial<ipc.MelodyReference> = {}): ipc.MelodyReference => ({
  id: 9,
  songId: "s1",
  source: "midi",
  status: "experimental",
  revision: 1,
  offsetMs: 0,
  notes: [{ startMs: 1000, durMs: 500, midi: 60, lyric: null }],
  ...over,
});

const renderRef = () =>
  render(
    <NavContext.Provider value={{ route: { name: "reference", songId: "s1" }, navigate: vi.fn(), replace, back: vi.fn() }}>
      <ToastProvider>
        <Reference songId="s1" />
      </ToastProvider>
    </NavContext.Provider>,
  );

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(ipc.listSongs).mockResolvedValue([song]);
});

describe("rollLayout", () => {
  const notes = [
    { startMs: 1000, durMs: 500, midi: 60, lyric: null },
    { startMs: 3000, durMs: 1000, midi: 64, lyric: null },
    { startMs: 20_000, durMs: 100, midi: 62, lyric: null },
  ];
  it("keeps only notes in the window and marks the active one", () => {
    const boxes = rollLayout(notes, 0, 1200, 2000, 4000); // window 0..5200 ms (span 6000)
    expect(boxes).toHaveLength(2);
    expect(boxes[0]).toMatchObject({ active: true });
    expect(boxes[0].x).toBeCloseTo((1000 - -800) / 6000);
    expect(boxes[0].w).toBeCloseTo(500 / 6000);
    expect(boxes[1].active).toBe(false);
    expect(boxes[1].y).toBeLessThan(boxes[0].y); // higher pitch drawn higher
  });
  it("positive offset delays notes", () => {
    const [b] = rollLayout(notes, 500, 1200, 2000, 4000);
    expect(b.active).toBe(false); // now starts at 1500
    expect(b.x).toBeCloseTo((1500 - -800) / 6000);
  });
  it("handles no notes", () => {
    expect(rollLayout([], 0, 0)).toEqual([]);
  });
});

describe("Reference", () => {
  it("imports JSON directly", async () => {
    vi.mocked(ipc.getReference).mockResolvedValue(null);
    vi.mocked(open).mockResolvedValue("C:/r/aquarela.json");
    vi.mocked(ipc.importReference).mockResolvedValue(ref({ source: "json" }));
    renderRef();
    expect(await screen.findByText("Sem referência")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Importar referência" }));
    expect(ipc.importReference).toHaveBeenCalledWith("s1", "C:/r/aquarela.json", null);
    expect(await screen.findByText("Referência experimental")).toBeInTheDocument();
  });

  it("asks which MIDI track is the voice when several have notes", async () => {
    vi.mocked(ipc.getReference).mockResolvedValue(null);
    vi.mocked(open).mockResolvedValue("C:/r/song.MID");
    vi.mocked(ipc.listMidiTracks).mockResolvedValue([
      { index: 0, name: "Piano", noteCount: 300 },
      { index: 1, name: "Vocal", noteCount: 120 },
      { index: 2, name: "Tempo", noteCount: 0 },
    ]);
    vi.mocked(ipc.importReference).mockResolvedValue(ref());
    renderRef();
    await userEvent.click(await screen.findByRole("button", { name: "Importar referência" }));
    expect(screen.getByRole("dialog", { name: "Qual trilha é a voz?" })).toBeInTheDocument();
    expect(screen.queryByText("Tempo")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Vocal/ }));
    expect(ipc.importReference).toHaveBeenCalledWith("s1", "C:/r/song.MID", 1);
  });

  it("imports a single-track MIDI without asking", async () => {
    vi.mocked(ipc.getReference).mockResolvedValue(null);
    vi.mocked(open).mockResolvedValue("C:/r/solo.mid");
    vi.mocked(ipc.listMidiTracks).mockResolvedValue([{ index: 3, name: null, noteCount: 40 }]);
    vi.mocked(ipc.importReference).mockResolvedValue(ref());
    renderRef();
    await userEvent.click(await screen.findByRole("button", { name: "Importar referência" }));
    expect(ipc.importReference).toHaveBeenCalledWith("s1", "C:/r/solo.mid", 3);
  });

  it("shows import errors and keeps the previous state", async () => {
    vi.mocked(ipc.getReference).mockResolvedValue(null);
    vi.mocked(open).mockResolvedValue("C:/r/bad.json");
    vi.mocked(ipc.importReference).mockRejectedValue({ code: "invalid_reference", message: "A referência não tem notas." });
    renderRef();
    await userEvent.click(await screen.findByRole("button", { name: "Importar referência" }));
    expect(await screen.findByText("A referência não tem notas.")).toBeInTheDocument();
    expect(screen.getByText("Sem referência")).toBeInTheDocument();
  });

  it("requires saving the offset before validating, then validates", async () => {
    vi.mocked(ipc.getReference).mockResolvedValue(ref());
    vi.mocked(ipc.setReferenceOffset).mockResolvedValue(ref({ offsetMs: 110, revision: 2 }));
    vi.mocked(ipc.setReferenceStatus).mockResolvedValue(ref({ offsetMs: 110, revision: 2, status: "validated" }));
    renderRef();
    const validate = await screen.findByRole("button", { name: "Validar alinhamento" });
    expect(validate).toBeEnabled();
    await userEvent.click(screen.getByRole("button", { name: "+100 ms" }));
    await userEvent.click(screen.getByRole("button", { name: "+10 ms" }));
    expect(screen.getByRole("slider", { name: "Offset das notas" })).toHaveAttribute("aria-valuetext", "+110 ms");
    expect(validate).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Salvar offset" }));
    expect(ipc.setReferenceOffset).toHaveBeenCalledWith(9, 110);
    expect(await screen.findByText("Revisão 2")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Validar alinhamento" }));
    expect(ipc.setReferenceStatus).toHaveBeenCalledWith(9, "validated");
    expect(await screen.findByText("Referência validada")).toBeInTheDocument();
  });

  it("clamps the offset slider to ±10 s", async () => {
    vi.mocked(ipc.getReference).mockResolvedValue(ref({ offsetMs: 9_950 }));
    renderRef();
    const slider = await screen.findByRole("slider", { name: "Offset das notas" });
    expect(slider).toHaveAttribute("min", "-10000");
    expect(slider).toHaveAttribute("max", "10000");
    await userEvent.click(screen.getByRole("button", { name: "+100 ms" }));
    expect(slider).toHaveAttribute("aria-valuetext", "+10000 ms");
    fireEvent.change(slider, { target: { value: "-200" } });
    expect(slider).toHaveAttribute("aria-valuetext", "-200 ms");
  });
});

describe("Gravar cantando (R8)", () => {
  // jsdom has no media pipeline: emulate play/pause state + events, and the frame callback.
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.mocked(ipc.startPerformance).mockResolvedValue({ competitive: false, reason: null });
    vi.mocked(ipc.syncPerformance).mockResolvedValue();
    vi.mocked(ipc.abortPerformance).mockResolvedValue();
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

  const outcome = (over: Partial<ipc.PerformanceOutcome>): ipc.PerformanceOutcome => ({
    performanceId: null,
    competitive: false,
    result: null,
    exploratory: null,
    personalBest: null,
    songRecord: null,
    ...over,
  });

  async function recordFromScratch() {
    renderRef();
    await userEvent.click(await screen.findByRole("button", { name: "Gravar cantando" }));
    expect(screen.getByText(/o app grava só as notas, nunca a sua voz/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Começar" }));
    expect(ipc.startPerformance).toHaveBeenCalledWith(
      { songId: "s1", roundId: null, participantId: null, referenceCapture: true },
      expect.any(Function),
    );
    expect(await screen.findByText("3")).toBeInTheDocument();
    for (let i = 0; i < 3; i++) await act(() => vi.advanceTimersByTimeAsync(1000));
    const badge = await screen.findByText("Gravando notas");
    expect(badge.closest('[role="status"]')).not.toBeNull(); // announced
    expect(screen.getByLabelText("Vídeo de Aquarela").parentElement).not.toContainElement(badge); // never over the picture
    expect(screen.getByLabelText("Vídeo de Aquarela")).toHaveProperty("paused", false);
  }

  it("records without a reference, then shows the sung notes as experimental with the usual actions", async () => {
    vi.mocked(ipc.getReference).mockResolvedValue(null);
    vi.mocked(ipc.finishPerformance).mockResolvedValue(
      outcome({ capturedReference: ref({ id: 12, source: "manual", notes: [{ startMs: 0, durMs: 400, midi: 62, lyric: null }] }) }),
    );
    await recordFromScratch();
    await userEvent.click(screen.getByRole("button", { name: "Parar" }));
    await vi.waitFor(() => expect(ipc.finishPerformance).toHaveBeenCalled());
    expect(ipc.syncPerformance).toHaveBeenLastCalledWith(expect.any(Number), false, false);
    expect(await screen.findByText("Cantada")).toBeInTheDocument();
    expect(screen.getByText("Referência experimental")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Validar alinhamento" })).toBeEnabled();
    expect(screen.getByRole("slider", { name: "Offset das notas" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Gravar de novo" })).toBeInTheDocument();
    expect(ipc.abortPerformance).not.toHaveBeenCalled();
  });

  it("explains why nothing was saved when too few notes came back (video ended)", async () => {
    vi.mocked(ipc.getReference).mockResolvedValue(null);
    vi.mocked(ipc.finishPerformance).mockResolvedValue(
      outcome({ capturedReference: null, reason: "Só 2 notas detectadas. Cante mais perto do microfone." }),
    );
    await recordFromScratch();
    fireEvent.ended(screen.getByLabelText("Vídeo de Aquarela"));
    expect(await screen.findByText("Só 2 notas detectadas. Cante mais perto do microfone.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Gravar de novo" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(screen.getByText("Sem referência")).toBeInTheDocument();
  });

  it("cancel while recording aborts the capture and keeps the current reference", async () => {
    vi.mocked(ipc.getReference).mockResolvedValue(ref({ status: "validated" }));
    renderRef();
    await userEvent.click(await screen.findByRole("button", { name: "Gravar cantando" }));
    expect(screen.getByText(/A referência atual fica no histórico/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Começar" }));
    for (let i = 0; i < 3; i++) await act(() => vi.advanceTimersByTimeAsync(1000));
    await userEvent.click(await screen.findByRole("button", { name: "Cancelar" }));
    await vi.waitFor(() => expect(ipc.abortPerformance).toHaveBeenCalledTimes(1));
    expect(ipc.finishPerformance).not.toHaveBeenCalled();
    expect(screen.getByText("Referência validada")).toBeInTheDocument();
  });
});
