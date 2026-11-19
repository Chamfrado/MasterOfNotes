import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MicMeter, PitchIndicator, RankingTable, ScoreBadge, midiToName } from "./game";

describe("midiToName", () => {
  it("maps MIDI numbers to note names", () => {
    expect(midiToName(60)).toBe("C4");
    expect(midiToName(69)).toBe("A4");
    expect(midiToName(61.4)).toBe("C#4");
    expect(midiToName(21)).toBe("A0");
  });
});

describe("MicMeter", () => {
  it("is a meter with level and clipping state", () => {
    render(<MicMeter frame={{ rmsDbfs: -12.4, f0Hz: 220, confidence: 0.9, voiced: true, clipped: true }} noiseFloorDbfs={-50} />);
    const meter = screen.getByRole("meter", { name: "Nível do microfone" });
    expect(meter).toHaveAttribute("aria-valuenow", "-12");
    expect(meter).toHaveAttribute("aria-valuetext", "-12 dBFS, saturando");
    expect(screen.getByText(/Saturando/)).toBeInTheDocument();
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
    expect(screen.getByRole("group", { name: "Alvo A4, acima 30 cents" })).toBeInTheDocument();
    expect(screen.getByText("Bom")).toBeInTheDocument();
  });
  it("handles undetected voice and no active note", () => {
    const { rerender } = render(
      <PitchIndicator
        feedback={{ tMs: 0, targetMidi: 60, sungMidi: null, centsError: null, label: "notDetected", runningScore: 0 }}
      />,
    );
    expect(screen.getByRole("group", { name: "Alvo C4, voz não detectada" })).toBeInTheDocument();
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
