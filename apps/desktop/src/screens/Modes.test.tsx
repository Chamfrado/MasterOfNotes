import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as ipc from "../lib/ipc";
import { NavContext } from "../nav";
import { Modes } from "./Modes";

vi.mock("../lib/ipc", () => ({ listSessions: vi.fn() }));

describe("Modes", () => {
  it("offers the four modes and resumes an active party after a restart", async () => {
    vi.mocked(ipc.listSessions).mockResolvedValue([
      { id: 7, mode: "party", name: "Sexta", status: "active", rounds: [{ status: "pending" }, { status: "completed" }] },
      { id: 8, mode: "duel", name: null, status: "completed", rounds: [] },
    ] as unknown as ipc.GameSession[]);
    const navigate = vi.fn();
    render(
      <NavContext.Provider value={{ route: { name: "modes" }, navigate, replace: vi.fn(), back: vi.fn() }}>
        <Modes />
      </NavContext.Provider>,
    );
    for (const m of ["Solo", "Festa", "Duelo", "Desafio"]) expect(screen.getByRole("button", { name: new RegExp(`^${m}`) })).toBeInTheDocument();
    expect(await screen.findByText("Sexta")).toBeInTheDocument();
    expect(screen.getByText(/falta 1 rodada/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Retomar" }));
    expect(navigate).toHaveBeenCalledWith({ name: "session", id: 7 });
    await userEvent.click(screen.getByRole("button", { name: /^Festa/ }));
    expect(navigate).toHaveBeenCalledWith({ name: "new-session", mode: "party" });
  });
});
