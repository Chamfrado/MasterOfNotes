import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components/ui";
import * as ipc from "../lib/ipc";
import { NavContext } from "../nav";
import { Library, searchKey } from "./Library";

const replace = vi.fn();
vi.mock("../lib/ipc", () => ({
  listSongs: vi.fn(),
  getSettings: vi.fn(),
  onLibraryChanged: vi.fn(() => Promise.resolve(() => {})),
  rescanLibrary: vi.fn(),
  setLibraryFolder: vi.fn(),
  updateSong: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
import { open } from "@tauri-apps/plugin-dialog";

const song = (over: Partial<ipc.Song>): ipc.Song => ({
  id: "x",
  path: "C:/k/x.mp4",
  fileName: "x.mp4",
  title: "x",
  artist: null,
  extension: "mp4",
  sizeBytes: 1,
  durationMs: null,
  playable: "unknown",
  available: true,
  referenceStatus: "none",
  ...over,
});
const settings = (libraryFolder: string | null) => ({ libraryFolder, recursiveScan: true }) as ipc.AppSettings;

const navigate = vi.fn();
function renderLibrary() {
  return render(
    <NavContext.Provider value={{ route: { name: "library" }, navigate, replace, back: vi.fn() }}>
      <ToastProvider>
        <Library />
      </ToastProvider>
    </NavContext.Provider>,
  );
}

beforeEach(() => vi.clearAllMocks());

describe("searchKey", () => {
  it("folds case and accents", () => {
    expect(searchKey("Canção DO Coração")).toBe("cancao do coracao");
  });
});

describe("Library", () => {
  it("searches without case or accents across title, artist and file name; hides unavailable", async () => {
    vi.mocked(ipc.listSongs).mockResolvedValue([
      song({ id: "1", title: "Canção da Manhã", artist: "Zé", referenceStatus: "validated", durationMs: 185_000 }),
      song({ id: "2", title: "Night Song", fileName: "noite_feliz.mp4", playable: "no" }),
      song({ id: "3", title: "Canção Sumida", available: false }),
    ]);
    vi.mocked(ipc.getSettings).mockResolvedValue(settings("C:/karaoke"));
    renderLibrary();

    expect(await screen.findByText("Canção da Manhã")).toBeInTheDocument();
    expect(screen.queryByText("Canção Sumida")).not.toBeInTheDocument();
    expect(screen.getByText("Referência validada")).toBeInTheDocument();
    expect(screen.getByText("3:05")).toBeInTheDocument();
    expect(screen.getByText("Formato incompatível")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cantar Night Song noite_feliz.mp4" })).toBeDisabled();

    const search = screen.getByRole("searchbox", { name: "Buscar músicas" });
    await userEvent.type(search, "CANCAO");
    expect(screen.getByText("Canção da Manhã")).toBeInTheDocument();
    expect(screen.queryByText("Night Song")).not.toBeInTheDocument();
    expect(screen.getByText("1 de 2 músicas")).toBeInTheDocument();

    await userEvent.clear(search);
    await userEvent.type(search, "noite");
    expect(screen.getByText("Night Song")).toBeInTheDocument();

    await userEvent.clear(search);
    await userEvent.type(search, "zzz");
    expect(screen.getByText(/Nada encontrado/)).toBeInTheDocument();
  });

  it("opens the player for a playable song", async () => {
    vi.mocked(ipc.listSongs).mockResolvedValue([song({ id: "1", title: "Aquarela" })]);
    vi.mocked(ipc.getSettings).mockResolvedValue(settings("C:/k"));
    renderLibrary();
    await userEvent.click(await screen.findByRole("button", { name: "Cantar Aquarela x.mp4" }));
    expect(navigate).toHaveBeenCalledWith({ name: "player", songId: "1" });
  });

  it("shows the first-run empty state and scans the chosen folder", async () => {
    vi.mocked(ipc.listSongs).mockResolvedValue([]);
    vi.mocked(ipc.getSettings).mockResolvedValue(settings(null));
    vi.mocked(open).mockResolvedValue("C:/karaoke");
    vi.mocked(ipc.setLibraryFolder).mockResolvedValue({ found: 3, added: 3, updated: 0, removed: 0, skipped: 1, durationMs: 10 });
    renderLibrary();

    expect(await screen.findByText("Comece escolhendo sua pasta de músicas")).toBeInTheDocument();
    await userEvent.click(screen.getAllByRole("button", { name: "Escolher pasta" })[0]);
    expect(ipc.setLibraryFolder).toHaveBeenCalledWith("C:/karaoke", true);
    expect(await screen.findByText("3 músicas encontradas (3 novas, 0 removidas)")).toBeInTheDocument();
  });

  it("shows the real backend error with retry", async () => {
    vi.mocked(ipc.listSongs).mockRejectedValueOnce({ code: "db", message: "Banco de dados indisponível." });
    vi.mocked(ipc.getSettings).mockResolvedValue(settings(null));
    renderLibrary();
    expect(await screen.findByText("Banco de dados indisponível.")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Não foi possível carregar a biblioteca" })).toBeInTheDocument();
    vi.mocked(ipc.listSongs).mockResolvedValueOnce([song({ title: "Voltou" })]);
    await userEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    expect(await screen.findByText("Voltou")).toBeInTheDocument();
  });

  it("edits title and artist through updateSong", async () => {
    vi.mocked(ipc.listSongs).mockResolvedValue([song({ id: "1", title: "Velho" })]);
    vi.mocked(ipc.getSettings).mockResolvedValue(settings("C:/k"));
    vi.mocked(ipc.updateSong).mockResolvedValue(song({ id: "1", title: "Novo" }));
    renderLibrary();
    await userEvent.click(await screen.findByRole("button", { name: "Editar Velho" }));
    const title = screen.getByRole("textbox", { name: "Título" });
    await userEvent.clear(title);
    expect(screen.getByRole("button", { name: "Salvar" })).toBeDisabled();
    await userEvent.type(title, "Novo");
    await userEvent.type(screen.getByRole("textbox", { name: "Artista" }), "Ana");
    await userEvent.click(screen.getByRole("button", { name: "Salvar" }));
    expect(ipc.updateSong).toHaveBeenCalledWith("1", { title: "Novo", artist: "Ana" });
  });
});
