import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components/ui";
import * as ipc from "../lib/ipc";
import { NavContext } from "../nav";
import { Session, challengeStatus, currentRound, duelResult } from "./Session";

vi.mock("../lib/ipc", () => ({
  getSession: vi.fn(),
  listParticipants: vi.fn(),
  listSongs: vi.fn(),
  setRoundStatus: vi.fn(),
  completeSession: vi.fn(),
}));

const round = (id: number, position: number, participantId: number | null, status: ipc.Round["status"], score: number | null = null, songId = "s1"): ipc.Round => ({
  id,
  position,
  songId,
  participantId,
  status,
  score,
});
const session = (over: Partial<ipc.GameSession>): ipc.GameSession => ({
  id: 5,
  mode: "party",
  name: "Festa da Ana",
  status: "active",
  config: { minRounds: 2, difficulty: "normal", seed: 1 },
  startedMs: 0,
  completedMs: null,
  rounds: [],
  standings: [],
  ...over,
});

describe("pure helpers", () => {
  it("currentRound is the first pending by position, none when not active", () => {
    const s = session({ rounds: [round(3, 3, 1, "pending"), round(1, 1, 1, "completed", 80), round(2, 2, 2, "skipped")] });
    expect(currentRound(s)?.id).toBe(3);
    expect(currentRound({ ...s, status: "completed" })).toBeNull();
  });

  it("duelResult: pending, win, tie", () => {
    expect(duelResult(session({ rounds: [round(1, 1, 1, "completed", 70), round(2, 2, 2, "pending")] }))).toEqual({ kind: "pending" });
    const win = duelResult(session({ rounds: [round(1, 1, 1, "completed", 70), round(2, 2, 2, "completed", 85.5)] }));
    expect(win).toMatchObject({ kind: "win", winnerId: 2 });
    expect(duelResult(session({ rounds: [round(1, 1, 1, "completed", 70), round(2, 2, 2, "completed", 70)] }))).toEqual({ kind: "tie", score: 70 });
  });

  it("challengeStatus: running, success, failure (skips count as played)", () => {
    const cfg = { difficulty: "normal" as const, seed: 1, targetScore: 75 };
    const running = challengeStatus(session({ mode: "challenge", config: cfg, rounds: [round(1, 1, null, "completed", 90), round(2, 2, null, "pending")] }));
    expect(running).toMatchObject({ state: "running", played: 1, total: 2, average: 90 });
    const ok = challengeStatus(session({ mode: "challenge", config: cfg, rounds: [round(1, 1, null, "completed", 90), round(2, 2, null, "completed", 60)] }));
    expect(ok).toMatchObject({ state: "success", average: 75 });
    const fail = challengeStatus(session({ mode: "challenge", config: cfg, rounds: [round(1, 1, null, "completed", 70), round(2, 2, null, "skipped")] }));
    expect(fail).toMatchObject({ state: "failure", played: 2, average: 70 });
  });
});

describe("Session screen (party)", () => {
  const navigate = vi.fn();
  const renderSession = () =>
    render(
      <NavContext.Provider value={{ route: { name: "session", id: 5 }, navigate, replace: vi.fn(), back: vi.fn() }}>
        <ToastProvider>
          <Session id={5} />
        </ToastProvider>
      </NavContext.Provider>,
    );

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.listParticipants).mockResolvedValue([
      { id: 1, name: "Ana" },
      { id: 2, name: "Bia" },
    ]);
    vi.mocked(ipc.listSongs).mockResolvedValue([{ id: "s1", title: "Aquarela", available: true } as ipc.Song]);
    vi.mocked(ipc.getSession).mockResolvedValue(
      session({
        rounds: [round(10, 1, 1, "completed", 80), round(11, 2, 2, "pending")],
        standings: [
          { participantId: 1, total: 80, average: 80, rounds: 1, eligible: false, rank: 1 },
          { participantId: 2, total: 0, average: 0, rounds: 0, eligible: false, rank: 2 },
        ],
      }),
    );
  });

  it("shows who sings now and opens the player with the round context", async () => {
    renderSession();
    expect(await screen.findByRole("heading", { name: "Bia canta Aquarela" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cantar" }));
    expect(navigate).toHaveBeenCalledWith({ name: "player", songId: "s1", round: { sessionId: 5, roundId: 11, participantId: 2 } });
  });

  it("shows partial standings with average/total and ineligible singers", async () => {
    renderSession();
    const table = await screen.findByRole("table", { name: "Ranking da festa" });
    expect(within(table).getByText("Inelegível: falta 1 rodada")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Ranking parcial" })).toBeInTheDocument();
  });

  it("skips a round through setRoundStatus", async () => {
    vi.mocked(ipc.setRoundStatus).mockResolvedValue(session({ rounds: [round(10, 1, 1, "completed", 80), round(11, 2, 2, "skipped")] }));
    renderSession();
    await userEvent.click(await screen.findByRole("button", { name: "Pular" }));
    expect(ipc.setRoundStatus).toHaveBeenCalledWith(11, "skipped");
    expect(await screen.findByRole("button", { name: "Voltar para a fila" })).toBeInTheDocument();
  });

  it("asks before ending with pending rounds, then shows the winner", async () => {
    vi.mocked(ipc.completeSession).mockResolvedValue(
      session({
        status: "completed",
        rounds: [round(10, 1, 1, "completed", 80), round(11, 2, 2, "pending")],
        standings: [{ participantId: 1, total: 80, average: 80, rounds: 1, eligible: true, rank: 1 }],
      }),
    );
    renderSession();
    await userEvent.click(await screen.findByRole("button", { name: "Encerrar" }));
    expect(screen.getByRole("dialog", { name: "Encerrar a partida?" })).toHaveTextContent("falta 1 rodada");
    await userEvent.click(screen.getByRole("button", { name: "Encerrar agora" }));
    expect(ipc.completeSession).toHaveBeenCalledWith(5);
    expect(await screen.findByText("Vencedor da festa")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Classificação final" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cantar" })).not.toBeInTheDocument();
  });

  it("shows the real backend error", async () => {
    vi.mocked(ipc.getSession).mockRejectedValue({ code: "not_found", message: "Partida não encontrada." });
    renderSession();
    expect(await screen.findByText("Partida não encontrada.")).toBeInTheDocument();
  });
});
