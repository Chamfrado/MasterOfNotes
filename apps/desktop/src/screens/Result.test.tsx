import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../lib/ipc";
import type { PerformanceOutcome } from "../lib/ipc";
import { NavContext } from "../nav";
import { Result, trainingAction } from "./Result";

vi.mock("../lib/ipc", () => ({ listSongs: vi.fn(), getSettings: vi.fn() }));

const replace = vi.fn();
const navigate = vi.fn();
const renderResult = (outcome: PerformanceOutcome) =>
  render(
    <NavContext.Provider value={{ route: { name: "home" }, navigate, replace, back: vi.fn() }}>
      <Result songId="s" outcome={outcome} />
    </NavContext.Provider>,
  );

const passed = { deviceId: null, videoVolume: 1, passed: true, atMs: 0 };
const settings = (leakCheck: ipc.LeakCheck | null) => ({ leakCheck, inputDeviceId: null, videoVolume: 0.8 }) as ipc.AppSettings;
const withSong = (referenceStatus: ipc.ReferenceStatus, leakCheck: ipc.LeakCheck | null = passed) => {
  vi.mocked(ipc.listSongs).mockResolvedValue([{ id: "s", referenceStatus } as ipc.Song]);
  vi.mocked(ipc.getSettings).mockResolvedValue(settings(leakCheck));
};
const scored = (over: Partial<PerformanceOutcome>): PerformanceOutcome => ({
  performanceId: 3,
  competitive: false,
  result: { score: 71, pitch: 0.7, rhythm: 0.7, sustain: 0.7, coverage: 0.7, notesTotal: 10, notesHit: 7, algorithmVersion: 3 },
  exploratory: null,
  personalBest: null,
  songRecord: null,
  ...over,
});

beforeEach(() => {
  vi.clearAllMocks();
  withSong("validated");
});

describe("trainingAction (derived from state, not from the reason text)", () => {
  it("points to the fix", () => {
    expect(trainingAction({ id: "s", referenceStatus: "none" }, settings(passed))).toMatchObject({ label: "Importar referência", to: { name: "reference", songId: "s" } });
    expect(trainingAction({ id: "s", referenceStatus: "experimental" }, settings(passed))).toMatchObject({ label: "Abrir referência" });
    expect(trainingAction({ id: "s", referenceStatus: "validated" }, settings(null))).toMatchObject({ label: "Fazer teste de vazamento", to: { name: "mic" } });
    expect(trainingAction({ id: "s", referenceStatus: "validated" }, settings(passed))).toBeNull(); // seek / dropouts / too short
  });
});

describe("Result", () => {
  it("shows the scored result", () => {
    renderResult({
      performanceId: 1,
      competitive: true,
      result: { score: 82.4, pitch: 0.9, rhythm: 0.8, sustain: 0.7, coverage: 0.6, notesTotal: 50, notesHit: 40, algorithmVersion: 1 },
      exploratory: null,
      personalBest: 82.4,
      songRecord: 95,
    });
    expect(screen.getByLabelText("82,4 pontos")).toBeInTheDocument();
    expect(screen.getByText("Novo recorde pessoal!")).toBeInTheDocument();
    expect(screen.getByText(/40 de 50 notas/)).toBeInTheDocument();
    expect(screen.getByText("Afinação").nextElementSibling).toHaveTextContent("90%"); // 0..1 shown as %
    expect(screen.getByText("Cobertura").nextElementSibling).toHaveTextContent("60%");
  });

  it("no reference: reason, import shortcut and exploratory metrics out of the ranking", async () => {
    withSong("none");
    renderResult({
      performanceId: null,
      competitive: false,
      result: null,
      exploratory: { voicedRatio: 0.62, pitchStability: 0.8 },
      reason: "Esta música não tem referência melódica.",
      personalBest: null,
      songRecord: null,
    });
    expect(screen.getByText("Esta música não tem referência melódica.")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Importar referência" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /Métricas exploratórias.*fora do ranking/ })).toBeInTheDocument();
    expect(screen.getByText("62%")).toBeInTheDocument();
    expect(screen.getByText("80%")).toBeInTheDocument();
    expect(screen.queryByText(/pontos/)).not.toBeInTheDocument();
  });

  it("practice run: badge, the backend reason and no shortcut when singing again is the fix", async () => {
    renderResult(scored({ reason: "Você pulou trechos da música, então esta apresentação virou treino." }));
    expect(screen.getByText("Treino — fora do ranking")).toBeInTheDocument();
    expect(screen.getByText("Você pulou trechos da música, então esta apresentação virou treino.")).toBeInTheDocument();
    await screen.findByRole("button", { name: "Cantar de novo" });
    await vi.waitFor(() => expect(ipc.getSettings).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: /Fazer teste|referência/ })).not.toBeInTheDocument();
  });

  it("pending leak test: shortcut to the mic screen", async () => {
    withSong("validated", null);
    renderResult(scored({ reason: "Faça o teste de vazamento do microfone para valer ranking." }));
    await userEvent.click(await screen.findByRole("button", { name: "Fazer teste de vazamento" }));
    expect(navigate).toHaveBeenCalledWith({ name: "mic" });
  });

  it("reference not validated: shortcut to the reference screen", async () => {
    withSong("experimental");
    renderResult(scored({ reason: "A referência ainda não foi validada." }));
    await userEvent.click(await screen.findByRole("button", { name: "Abrir referência" }));
    expect(navigate).toHaveBeenCalledWith({ name: "reference", songId: "s" });
  });

  it("competitive run shows no practice notice", async () => {
    renderResult(scored({ competitive: true, reason: null }));
    expect(screen.queryByText("Treino — fora do ranking")).not.toBeInTheDocument();
  });
});
