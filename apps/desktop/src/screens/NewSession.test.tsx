import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components/ui";
import * as ipc from "../lib/ipc";
import { NavContext } from "../nav";
import { NewSession, validateSession, type SessionForm } from "./NewSession";

vi.mock("../lib/ipc", () => ({
  listParticipants: vi.fn(),
  listSongs: vi.fn(),
  createParticipant: vi.fn(),
  createSession: vi.fn(),
}));

const form = (over: Partial<SessionForm>): SessionForm => ({
  name: "",
  participantIds: [],
  songIds: [],
  order: "shuffle",
  minRounds: "",
  targetScore: "80",
  ...over,
});

describe("validateSession", () => {
  it("party: name, ≥ 2 singers, ≥ 1 song, valid minRounds", () => {
    expect(validateSession("party", form({}))).toEqual([
      "Dê um nome à festa.",
      "Escolha pelo menos 2 cantores.",
      "Escolha pelo menos 1 música.",
    ]);
    expect(validateSession("party", form({ name: "F", participantIds: [1, 2], songIds: ["a"], minRounds: "0" }))).toEqual([
      "Mínimo de rodadas deve ser um número inteiro a partir de 1.",
    ]);
    expect(validateSession("party", form({ name: "F", participantIds: [1, 2], songIds: ["a"], minRounds: "2" }))).toEqual([]);
  });
  it("duel: exactly 2 singers and 1 song", () => {
    expect(validateSession("duel", form({ participantIds: [1], songIds: ["a", "b"] }))).toEqual([
      "Escolha exatamente 2 cantores.",
      "Escolha exatamente 1 música.",
    ]);
  });
  it("solo: 0–1 singer, 1 song; challenge needs a 0–100 target", () => {
    expect(validateSession("solo", form({ songIds: ["a"] }))).toEqual([]);
    expect(validateSession("solo", form({ participantIds: [1, 2], songIds: ["a"] }))).toEqual(["Escolha no máximo 1 cantor."]);
    expect(validateSession("challenge", form({ songIds: ["a"], targetScore: "101" }))).toEqual(["Meta de pontuação deve ficar entre 0 e 100."]);
  });
});

describe("NewSession (party)", () => {
  const replace = vi.fn();
  const song = (id: string, title: string, referenceStatus: ipc.ReferenceStatus) =>
    ({ id, title, artist: null, fileName: `${id}.mp4`, available: true, playable: "yes", referenceStatus }) as ipc.Song;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.listParticipants).mockResolvedValue([{ id: 1, name: "Ana" }]);
    vi.mocked(ipc.listSongs).mockResolvedValue([song("a", "Aquarela", "validated"), song("b", "Bossa", "experimental"), song("c", "Choro", "validated")]);
  });

  it("only offers validated songs, adds a singer inline and creates the party", async () => {
    vi.mocked(ipc.createParticipant).mockResolvedValue({ id: 2, name: "Bia" });
    vi.mocked(ipc.createSession).mockResolvedValue({ id: 42 } as ipc.GameSession);
    render(
      <NavContext.Provider value={{ route: { name: "new-session", mode: "party" }, navigate: vi.fn(), replace, back: vi.fn() }}>
        <ToastProvider>
          <NewSession mode="party" />
        </ToastProvider>
      </NavContext.Provider>,
    );
    expect(await screen.findByText("Aquarela")).toBeInTheDocument();
    expect(screen.queryByText("Bossa")).not.toBeInTheDocument();
    expect(screen.getByText(/1 música não aparece porque ainda não tem referência validada/)).toBeInTheDocument();
    const create = screen.getByRole("button", { name: "Criar festa" });
    expect(create).toBeDisabled();

    await userEvent.type(screen.getByRole("textbox", { name: "Nome da festa" }), "Sexta");
    await userEvent.click(screen.getByRole("button", { name: "Ana" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Novo cantor" }), "Bia{Enter}");
    expect(ipc.createParticipant).toHaveBeenCalledWith("Bia");
    expect(await screen.findByRole("button", { name: "Bia", pressed: true })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("checkbox", { name: /Choro/ }));
    await userEvent.click(screen.getByRole("checkbox", { name: /Aquarela/ }));
    await userEvent.click(screen.getByRole("radio", { name: /Na ordem em que escolhi/ }));
    await userEvent.type(screen.getByRole("textbox", { name: /Mínimo de rodadas/ }), "2");
    expect(create).toBeEnabled();
    await userEvent.click(create);
    expect(ipc.createSession).toHaveBeenCalledWith({
      mode: "party",
      name: "Sexta",
      participantIds: [1, 2],
      songIds: ["c", "a"],
      order: "manual",
      minRounds: 2,
      targetScore: undefined,
    });
    expect(replace).toHaveBeenCalledWith({ name: "session", id: 42 });
  });

  it("shows a duplicate-name error from Rust under the field", async () => {
    vi.mocked(ipc.createParticipant).mockRejectedValue({ code: "participant_exists", message: "Já existe um cantor com esse nome." });
    render(
      <NavContext.Provider value={{ route: { name: "new-session", mode: "party" }, navigate: vi.fn(), replace, back: vi.fn() }}>
        <ToastProvider>
          <NewSession mode="party" />
        </ToastProvider>
      </NavContext.Provider>,
    );
    await userEvent.type(await screen.findByRole("textbox", { name: "Novo cantor" }), "ana{Enter}");
    expect(await screen.findByRole("textbox", { name: "Novo cantor" })).toHaveAccessibleDescription("Já existe um cantor com esse nome.");
  });
});
