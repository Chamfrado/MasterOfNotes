import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { PerformanceOutcome } from "../lib/ipc";
import { NavContext } from "../nav";
import { Result } from "./Result";

const renderResult = (outcome: PerformanceOutcome) =>
  render(
    <NavContext.Provider value={{ route: { name: "home" }, navigate: vi.fn(), back: vi.fn() }}>
      <Result songId="s" outcome={outcome} />
    </NavContext.Provider>,
  );

describe("Result", () => {
  it("shows the scored result", () => {
    renderResult({
      performanceId: 1,
      competitive: true,
      result: { score: 82.4, pitch: 90, rhythm: 80, sustain: 70, coverage: 60, notesTotal: 50, notesHit: 40, algorithmVersion: 1 },
      exploratory: null,
      personalBest: 82.4,
      songRecord: 95,
    });
    expect(screen.getByLabelText("82,4 pontos")).toBeInTheDocument();
    expect(screen.getByText("Novo recorde pessoal!")).toBeInTheDocument();
    expect(screen.getByText(/40 de 50 notas/)).toBeInTheDocument();
  });

  it("shows exploratory metrics, out of the ranking, when there is no reference", () => {
    renderResult({
      performanceId: null,
      competitive: false,
      result: null,
      exploratory: { voicedRatio: 0.62, pitchStability: 0.8 },
      personalBest: null,
      songRecord: null,
    });
    expect(screen.getByRole("heading", { name: /Métricas exploratórias.*fora do ranking/ })).toBeInTheDocument();
    expect(screen.getByText("62%")).toBeInTheDocument();
    expect(screen.getByText("80%")).toBeInTheDocument();
    expect(screen.queryByText(/pontos/)).not.toBeInTheDocument();
  });
});
