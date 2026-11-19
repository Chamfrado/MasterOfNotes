import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components/ui";
import * as ipc from "../lib/ipc";
import { NavContext } from "../nav";
import { SongDetails } from "./SongDetails";

vi.mock("../lib/ipc", () => ({
  listSongs: vi.fn(),
  getSettings: vi.fn(),
  getLeaderboard: vi.fn(),
  listParticipants: vi.fn(),
  listPerformances: vi.fn(),
  updateSong: vi.fn(),
}));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({}) })); // Player module import

const song = (over: Partial<ipc.Song> = {}): ipc.Song => ({
  id: "s1",
  path: "C:/k/aquarela.mp4",
  fileName: "aquarela.mp4",
  title: "Aquarela",
  artist: null,
  extension: "mp4",
  sizeBytes: 3_145_728,
  durationMs: 185_000,
  playable: "yes",
  available: true,
  referenceStatus: "validated",
  ...over,
});
const rows: ipc.LeaderboardRow[] = Array.from({ length: 7 }, (_, i) => ({
  rank: i + 1,
  participantName: ["Ana", "Bia", "Caio", "Duda", "Edu", "Fê", "Gil"][i],
  score: 95 - i * 3,
  createdMs: 0,
  performanceId: i + 1,
}));
const navigate = vi.fn();
const renderDetails = () =>
  render(
    <NavContext.Provider value={{ route: { name: "song", songId: "s1" }, navigate, replace: vi.fn(), back: vi.fn() }}>
      <ToastProvider>
        <SongDetails songId="s1" />
      </ToastProvider>
    </NavContext.Provider>,
  );

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(ipc.listSongs).mockResolvedValue([song()]);
  vi.mocked(ipc.getSettings).mockResolvedValue({ difficulty: "hard" } as ipc.AppSettings);
  vi.mocked(ipc.getLeaderboard).mockResolvedValue(rows);
  vi.mocked(ipc.listPerformances).mockResolvedValue([]);
  vi.mocked(ipc.listParticipants).mockResolvedValue([
    { id: 1, name: "Bia" },
    { id: 2, name: "Zeca" },
  ]);
});

describe("SongDetails", () => {
  it("shows file info, reference state, top 5 for the current difficulty and the actions", async () => {
    renderDetails();
    expect(await screen.findByRole("heading", { level: 1, name: "Aquarela" })).toBeInTheDocument();
    expect(screen.getByText("aquarela.mp4")).toBeInTheDocument();
    expect(screen.getByText("3:05")).toBeInTheDocument();
    expect(screen.getByText("3 MB")).toBeInTheDocument();
    expect(screen.getByText("Referência validada")).toBeInTheDocument();
    await vi.waitFor(() => expect(ipc.getLeaderboard).toHaveBeenCalledWith("s1", "hard"));
    const table = await screen.findByRole("table", { name: "Top 5 de Aquarela" });
    expect(within(table).getAllByRole("row")).toHaveLength(6); // header + 5
    expect(screen.getByText(/Recorde da música/)).toHaveTextContent("Ana");

    await userEvent.click(screen.getByRole("button", { name: "Cantar" }));
    expect(navigate).toHaveBeenCalledWith({ name: "player", songId: "s1" });
    await userEvent.click(screen.getByRole("button", { name: "Ranking completo" }));
    expect(navigate).toHaveBeenCalledWith({ name: "ranking", songId: "s1" });
    await userEvent.click(screen.getByRole("button", { name: "Abrir referência" }));
    expect(navigate).toHaveBeenCalledWith({ name: "reference", songId: "s1" });
  });

  it("shows a singer's personal best from the leaderboard", async () => {
    renderDetails();
    const select = await screen.findByRole("combobox", { name: "Recorde pessoal de" });
    await userEvent.selectOptions(select, "Bia");
    expect(screen.getByText("Bia: 92 pontos (2º lugar).")).toBeInTheDocument();
    await userEvent.selectOptions(select, "Zeca");
    expect(screen.getByText("Zeca ainda não tem pontuação no ranking desta música.")).toBeInTheDocument();
  });

  it("explains an unplayable file and blocks Cantar", async () => {
    vi.mocked(ipc.listSongs).mockResolvedValue([song({ playable: "no", referenceStatus: "none" })]);
    renderDetails();
    expect(await screen.findByText(/H\.264 e áudio AAC/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cantar" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Importar referência" })).toBeInTheDocument();
  });

  it("edits title and artist", async () => {
    vi.mocked(ipc.updateSong).mockResolvedValue(song({ title: "Aquarela (ao vivo)", artist: "Toquinho" }));
    renderDetails();
    const save = await screen.findByRole("button", { name: "Salvar alterações" });
    expect(save).toBeDisabled(); // nothing changed yet
    await userEvent.type(screen.getByRole("textbox", { name: "Título" }), " (ao vivo)");
    await userEvent.type(screen.getByRole("textbox", { name: "Artista" }), "Toquinho");
    await userEvent.click(save);
    expect(ipc.updateSong).toHaveBeenCalledWith("s1", { title: "Aquarela (ao vivo)", artist: "Toquinho" });
    expect(await screen.findByRole("heading", { level: 1, name: "Aquarela (ao vivo)" })).toBeInTheDocument();
  });

  it("keeps the page usable when only the records fail", async () => {
    vi.mocked(ipc.getLeaderboard).mockRejectedValue({ code: "db", message: "Banco ocupado." });
    renderDetails();
    expect(await screen.findByText("Banco ocupado.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cantar" })).toBeEnabled();
  });

  it("lists the last 10 performances with practice and older-version flags", async () => {
    const row = (over: Partial<ipc.PerformanceRow>): ipc.PerformanceRow => ({
      performanceId: 1,
      songId: "s1",
      songTitle: "Aquarela",
      participantName: "Ana",
      score: 88.4,
      competitive: true,
      algorithmVersion: 2,
      referenceRevision: 2,
      difficulty: "normal",
      createdMs: 0,
      isCurrentKey: true,
      ...over,
    });
    vi.mocked(ipc.listPerformances).mockResolvedValue([
      row({}),
      row({ performanceId: 2, participantName: null, competitive: false }),
      row({ performanceId: 3, isCurrentKey: false, score: 70 }),
    ]);
    renderDetails();
    const list = within(await screen.findByRole("region", { name: "Suas apresentações" }));
    expect(ipc.listPerformances).toHaveBeenCalledWith({ songId: "s1", limit: 10 });
    await vi.waitFor(() => expect(list.getAllByRole("listitem")).toHaveLength(3));
    const items = list.getAllByRole("listitem");
    expect(items[0]).toHaveTextContent("88,4");
    expect(items[0]).not.toHaveTextContent("Treino");
    expect(items[1]).toHaveTextContent("Anônimo");
    expect(items[1]).toHaveTextContent("Treino");
    expect(items[2]).toHaveTextContent("Versão anterior");
  });

  it("turns a missing history command into a discreet empty state", async () => {
    vi.mocked(ipc.listPerformances).mockRejectedValue("Command list_performances not found");
    renderDetails();
    expect(await screen.findByText(/Histórico indisponível no momento \(Command list_performances not found\)/)).toBeInTheDocument();
    expect(screen.queryByRole("listitem")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cantar" })).toBeEnabled();
  });

  it("shows the real error when the song list fails", async () => {
    vi.mocked(ipc.listSongs).mockRejectedValue({ code: "db", message: "Banco indisponível." });
    renderDetails();
    expect(await screen.findByText("Banco indisponível.")).toBeInTheDocument();
  });
});
