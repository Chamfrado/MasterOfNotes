import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MicMeter, PitchIndicator, RankingTable, ScoreBadge, StandingsTable, hzToMidi, midiToName } from "./game";

describe("midiToName", () => {
  it("maps MIDI numbers to note names", () => {
    expect(midiToName(60)).toBe("Dó4");
    expect(midiToName(69)).toBe("Lá4");
    expect(midiToName(61.4)).toBe("Dó#4");
    expect(midiToName(62)).toBe("Ré4");
    expect(midiToName(65)).toBe("Fá4");
    expect(midiToName(71)).toBe("Si4");
    expect(midiToName(72)).toBe("Dó5");
    expect(midiToName(21)).toBe("Lá0");
  });
  it("converts Hz to MIDI", () => {
    expect(hzToMidi(440)).toBe(69);
    expect(hzToMidi(261.63)).toBeCloseTo(60, 2);
  });
});

describe("MicMeter", () => {
  it("is a meter with level and clipping state", () => {
    render(<MicMeter frame={{ rmsDbfs: -12.4, f0Hz: 220, confidence: 0.9, voiced: true, clipped: true }} noiseFloorDbfs={-50} />);
    const meter = screen.getByRole("meter", { name: "Nível do microfone" });
    expect(meter).toHaveAttribute("aria-valuenow", "-12");
    expect(meter).toHaveAttribute("aria-valuetext", "-12 dBFS, saturando");
    expect(screen.getByText(/Saturando/)).toBeInTheDocument();
    expect(screen.getByText("Nota Lá3 · 220 Hz")).toBeInTheDocument();
  });
  it("clamps and reports no signal", () => {
    render(<MicMeter frame={null} />);
    const meter = screen.getByRole("meter");
    expect(meter).toHaveAttribute("aria-valuenow", "-60");
    expect(meter).toHaveAttribute("aria-valuetext", "sem sinal");
  });
});

describe("PitchIndicator", () => {
  it("describes target and deviation", () => {
    render(
      <PitchIndicator
        feedback={{ tMs: 1000, targetMidi: 69, sungMidi: 69.3, centsError: 30, label: "good", runningScore: 50 }}
      />,
    );
    expect(screen.getByRole("group", { name: "Alvo Lá4, acima 30 cents" })).toBeInTheDocument();
    expect(screen.getByText("Bom")).toBeInTheDocument();
  });
  it("handles undetected voice and no active note", () => {
    const { rerender } = render(
      <PitchIndicator
        feedback={{ tMs: 0, targetMidi: 60, sungMidi: null, centsError: null, label: "notDetected", runningScore: 0 }}
      />,
    );
    expect(screen.getByRole("group", { name: "Alvo Dó4, voz não detectada" })).toBeInTheDocument();
    rerender(<PitchIndicator feedback={null} />);
    expect(screen.getByText(/Aguardando/)).toBeInTheDocument();
  });
});

describe("ScoreBadge", () => {
  it("formats in pt-BR and flags practice runs", () => {
    render(<ScoreBadge score={87.25} competitive={false} />);
    expect(screen.getByLabelText("87,3 pontos, treino")).toHaveTextContent("87,3");
    expect(screen.getByText("Treino")).toBeInTheDocument();
  });
});

describe("RankingTable", () => {
  it("renders a captioned table with row headers and highlight", () => {
    render(
      <RankingTable
        caption="Ranking de Música X"
        highlightId={2}
        rows={[
          { rank: 1, participantName: "Ana", score: 91.5, createdMs: 0, performanceId: 1 },
          { rank: 2, participantName: "Bia", score: 80, createdMs: 0, performanceId: 2 },
        ]}
      />,
    );
    const table = screen.getByRole("table", { name: "Ranking de Música X" });
    const rows = within(table).getAllByRole("row");
    expect(rows).toHaveLength(3);
    expect(within(rows[2]).getByRole("rowheader", { name: "Bia" })).toBeInTheDocument();
    expect(rows[2]).toHaveAttribute("aria-current", "true");
  });
  it("shows an empty state", () => {
    render(<RankingTable caption="x" rows={[]} />);
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.getByText(/Ninguém pontuou/)).toBeInTheDocument();
  });
});

describe("StandingsTable", () => {
  // SPEC-008 criterion: A 3 rounds (avg 80, total 240), B 2 rounds (avg 90, total 180).
  const names = new Map([
    [1, "Ana"],
    [2, "Bia"],
    [3, "Caio"],
  ]);
  it("shows average and total, eligible first, ineligible flagged with missing rounds", () => {
    render(
      <StandingsTable
        caption="Classificação"
        minRounds={3}
        names={names}
        standings={[
          { participantId: 2, total: 180, average: 90, rounds: 2, eligible: false, rank: 2 },
          { participantId: 1, total: 240, average: 80, rounds: 3, eligible: true, rank: 1 },
          { participantId: 3, total: 0, average: 0, rounds: 0, eligible: false, rank: 3 },
        ]}
      />,
    );
    const rows = within(screen.getByRole("table", { name: "Classificação" })).getAllByRole("row");
    expect(within(rows[1]).getByRole("rowheader")).toHaveTextContent("Ana");
    expect(rows[1]).toHaveTextContent("80");
    expect(rows[1]).toHaveTextContent("240");
    expect(within(rows[2]).getByRole("rowheader")).toHaveTextContent("Bia");
    expect(rows[2]).toHaveTextContent("Inelegível: falta 1 rodada");
    expect(rows[3]).toHaveTextContent("Inelegível: faltam 3 rodadas");
  });
  it("has an empty state", () => {
    render(<StandingsTable caption="x" minRounds={1} names={names} standings={[]} />);
    expect(screen.getByText("Ninguém cantou ainda.")).toBeInTheDocument();
  });
});
