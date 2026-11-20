import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components/ui";
import * as ipc from "../lib/ipc";
import { NavContext } from "../nav";
import { Home } from "./Home";
import { Settings } from "./Settings";
import { Updates } from "./Updates";

vi.mock("../lib/ipc", () => ({
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
  listKnownSeparationModels: vi.fn(),
  installSeparationModel: vi.fn(),
  removeSeparationModel: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
import { open } from "@tauri-apps/plugin-dialog";
vi.mock("@tauri-apps/api/app", () => ({ getVersion: () => Promise.resolve("0.1.0") }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
import { check, type Update } from "@tauri-apps/plugin-updater";
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
import { openUrl } from "@tauri-apps/plugin-opener";

const base = {
  libraryFolder: "C:/k",
  recursiveScan: true,
  inputDeviceId: null,
  noiseFloorDbfs: -50,
  latencyOffsetMs: 0,
  difficulty: "normal",
  videoVolume: 0.8,
  reducedMotion: "system",
  feedbackOverlay: "full",
  autoCheckUpdates: false,
  leakCheck: null,
  tvMode: false,
  updateConsentAsked: false,
} as ipc.AppSettings;
const model: ipc.KnownSeparationModel = {
  id: "ab12cd34",
  name: "Separador Vocal X",
  fileName: "vocals-x.onnx",
  sizeBytes: 67_108_864,
  sourceUrl: "https://example.org/vocals-x",
  license: "MIT; pesos de terceiros",
};
const nav = { route: { name: "settings" as const }, navigate: vi.fn(), replace: vi.fn(), back: vi.fn() };
const wrap = (ui: React.ReactNode) =>
  render(
    <NavContext.Provider value={nav}>
      <ToastProvider>{ui}</ToastProvider>
    </NavContext.Provider>,
  );

beforeEach(() => {
  vi.clearAllMocks();
  delete document.documentElement.dataset.tv;
  delete document.documentElement.dataset.reducedMotion;
  vi.mocked(ipc.getSettings).mockResolvedValue(base);
  vi.mocked(ipc.listKnownSeparationModels).mockResolvedValue([model]);
});

describe("Settings → Referência automática (SPEC-012 R1)", () => {
  it("lists the known model with license and size, links to its source, and needs the terms before choosing a file", async () => {
    vi.mocked(open).mockResolvedValue("C:/Downloads/vocals-x.onnx");
    vi.mocked(ipc.installSeparationModel).mockResolvedValue({ id: "ab12cd34", name: "Separador Vocal X", acceptedMs: 0 });
    wrap(<Settings />);
    expect(await screen.findByText("Separador Vocal X")).toBeInTheDocument();
    expect(screen.getByText("Licença: MIT; pesos de terceiros")).toBeInTheDocument();
    expect(screen.getByText(/64 MB/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Abrir página do modelo" }));
    expect(openUrl).toHaveBeenCalledWith("https://example.org/vocals-x");

    const choose = screen.getByRole("button", { name: "Escolher arquivo baixado…" });
    expect(choose).toBeDisabled();
    await userEvent.click(screen.getByRole("checkbox", { name: /Li e aceito os termos da fonte/ }));
    await userEvent.click(choose);
    expect(open).toHaveBeenCalledWith(expect.objectContaining({ filters: [{ name: "Modelo ONNX", extensions: ["onnx"] }] }));
    expect(ipc.installSeparationModel).toHaveBeenCalledWith("C:/Downloads/vocals-x.onnx", true);
    expect(await screen.findByText(/instalado em/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remover" })).toBeInTheDocument();
  });

  it("shows Rust's refusal of an unknown file and stays uninstalled", async () => {
    vi.mocked(open).mockResolvedValue("C:/Downloads/outro.onnx");
    vi.mocked(ipc.installSeparationModel).mockRejectedValue({ code: "unknown_model", message: "Este arquivo não é um modelo conhecido." });
    wrap(<Settings />);
    await userEvent.click(await screen.findByRole("checkbox", { name: /Li e aceito/ }));
    await userEvent.click(screen.getByRole("button", { name: "Escolher arquivo baixado…" }));
    expect(await screen.findByText("Este arquivo não é um modelo conhecido.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Remover" })).not.toBeInTheDocument();
  });

  it("shows the installed model and removes it", async () => {
    vi.mocked(ipc.getSettings).mockResolvedValue({
      ...base,
      separationModel: { id: "ab12cd34", name: "Separador Vocal X", acceptedMs: Date.UTC(2026, 10, 20, 12) },
    });
    vi.mocked(ipc.removeSeparationModel).mockResolvedValue();
    wrap(<Settings />);
    expect(await screen.findByText("· instalado em 20/11/2026")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Remover" }));
    expect(ipc.removeSeparationModel).toHaveBeenCalled();
    expect(await screen.findByRole("button", { name: "Escolher arquivo baixado…" })).toBeDisabled();
  });

  it("focuses the section when opened from the Reference screen", async () => {
    wrap(<Settings section="autoReference" />);
    await vi.waitFor(() => expect(screen.getByRole("heading", { name: "Referência automática" })).toHaveFocus());
  });
});

describe("Settings", () => {
  it("persists game settings through updateSettings", async () => {
    vi.mocked(ipc.updateSettings).mockImplementation((p) => Promise.resolve({ ...base, ...p }));
    wrap(<Settings />);
    await userEvent.selectOptions(await screen.findByRole("combobox", { name: "Dificuldade" }), "hard");
    expect(ipc.updateSettings).toHaveBeenCalledWith({ difficulty: "hard" });
    await userEvent.click(screen.getByRole("switch", { name: /Procurar também nas subpastas/ }));
    expect(ipc.updateSettings).toHaveBeenCalledWith({ recursiveScan: false });
    await userEvent.click(screen.getByRole("switch", { name: /Verificar atualizações ao abrir/ }));
    expect(ipc.updateSettings).toHaveBeenCalledWith({ autoCheckUpdates: true });
  });

  it("applies reduced motion and TV mode to <html> immediately", async () => {
    vi.mocked(ipc.updateSettings).mockImplementation((p) => Promise.resolve({ ...base, ...p }));
    wrap(<Settings />);
    await userEvent.selectOptions(await screen.findByRole("combobox", { name: "Reduzir movimento" }), "on");
    expect(document.documentElement.dataset.reducedMotion).toBe("on");
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Reduzir movimento" }), "system");
    expect(document.documentElement.dataset.reducedMotion).toBeUndefined();
    await userEvent.click(screen.getByRole("switch", { name: /Modo TV/ }));
    expect(document.documentElement.dataset.tv).toBe("on");
    expect(ipc.updateSettings).toHaveBeenCalledWith({ tvMode: true });
  });

  it("undoes TV mode on screen when Rust rejects it", async () => {
    vi.mocked(ipc.updateSettings).mockRejectedValue({ code: "invalid_setting", message: "Valor inválido." });
    wrap(<Settings />);
    await userEvent.click(await screen.findByRole("switch", { name: /Modo TV/ }));
    expect(await screen.findByText("Valor inválido.")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: /Modo TV/ })).not.toBeChecked();
    expect(document.documentElement.dataset.tv).toBeUndefined();
  });

  it("reverts and reports when Rust rejects a value", async () => {
    vi.mocked(ipc.updateSettings).mockRejectedValue({ code: "invalid_setting", message: "Valor inválido." });
    wrap(<Settings />);
    const select = await screen.findByRole("combobox", { name: "Dificuldade" });
    await userEvent.selectOptions(select, "easy");
    expect(await screen.findByText("Valor inválido.")).toBeInTheDocument();
    expect(select).toHaveValue("normal");
  });
});

describe("Home update consent", () => {
  it("asks once; yes turns on autoCheckUpdates", async () => {
    vi.mocked(ipc.updateSettings).mockResolvedValue({ ...base, autoCheckUpdates: true, updateConsentAsked: true });
    const { unmount } = wrap(<Home />);
    await userEvent.click(await screen.findByRole("button", { name: "Sim, verificar" }));
    expect(ipc.updateSettings).toHaveBeenCalledWith({ autoCheckUpdates: true, updateConsentAsked: true });
    expect(screen.queryByRole("region", { name: "Atualizações automáticas" })).not.toBeInTheDocument();
    unmount();
    vi.mocked(ipc.getSettings).mockResolvedValue({ ...base, updateConsentAsked: true });
    wrap(<Home />);
    await screen.findByRole("heading", { name: "Preparação" });
    expect(screen.queryByRole("region", { name: "Atualizações automáticas" })).not.toBeInTheDocument();
  });
  it("'Agora não' keeps it off and does not ask again", async () => {
    vi.mocked(ipc.updateSettings).mockResolvedValue({ ...base, updateConsentAsked: true });
    wrap(<Home />);
    await userEvent.click(await screen.findByRole("button", { name: "Agora não" }));
    expect(ipc.updateSettings).toHaveBeenCalledWith({ updateConsentAsked: true });
    expect(screen.queryByRole("region", { name: "Atualizações automáticas" })).not.toBeInTheDocument();
  });
  it("is not shown when settings say it was already asked, and TV mode follows settings", async () => {
    vi.mocked(ipc.getSettings).mockResolvedValue({ ...base, updateConsentAsked: true, tvMode: true });
    wrap(<Home />);
    await screen.findByRole("heading", { name: "Preparação" });
    expect(screen.queryByRole("region", { name: "Atualizações automáticas" })).not.toBeInTheDocument();
    wrap(<Settings />);
    expect(await screen.findByRole("switch", { name: /Modo TV/ })).toBeChecked();
  });
});

describe("Updates", () => {
  it("says when the app is current", async () => {
    vi.mocked(check).mockResolvedValue(null);
    wrap(<Updates />);
    expect(await screen.findByText("Você já está na versão mais recente.")).toBeInTheDocument();
    expect(screen.getByText("Versão instalada: 0.1.0")).toBeInTheDocument();
  });

  it("shows notes, downloads with progress, and installs only after confirmation", async () => {
    const update = {
      version: "0.2.0",
      body: "Modo dueto",
      download: vi.fn(async (on: (e: unknown) => void) => {
        on({ event: "Started", data: { contentLength: 2_097_152 } });
        on({ event: "Progress", data: { chunkLength: 1_048_576 } });
        on({ event: "Progress", data: { chunkLength: 1_048_576 } });
        on({ event: "Finished" });
      }),
      install: vi.fn(() => Promise.resolve()),
      close: vi.fn(() => Promise.resolve()),
    };
    vi.mocked(check).mockResolvedValue(update as unknown as Update);
    wrap(<Updates />);
    expect(await screen.findByRole("heading", { name: "Versão 0.2.0 disponível" })).toBeInTheDocument();
    expect(screen.getByText("Modo dueto")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Baixar atualização" }));
    expect(update.download).toHaveBeenCalled();
    const dialog = await screen.findByRole("dialog", { name: "Instalar agora?" });
    expect(update.install).not.toHaveBeenCalled();
    await userEvent.click(within(dialog).getByRole("button", { name: "Instalar e reiniciar" }));
    expect(update.install).toHaveBeenCalled();
  });

  it("says updates are not configured when the build has no updater (no pubkey)", async () => {
    vi.mocked(check).mockRejectedValue("plugin updater not found");
    wrap(<Updates />);
    expect(await screen.findByText(/Atualizações automáticas ainda não estão configuradas nesta versão/)).toBeInTheDocument();
    expect(screen.queryByText(/plugin updater not found/)).not.toBeInTheDocument();
  });

  it("explains failures (network / signature) and keeps the app usable", async () => {
    vi.mocked(check).mockRejectedValue("error sending request for url");
    wrap(<Updates />);
    expect(await screen.findByText(/O app continua funcionando normalmente\./)).toBeInTheDocument();
    expect(screen.getByText(/error sending request for url/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Tentar de novo" })).toBeInTheDocument();
  });

  it("treats a missing release manifest (404, pre-releases only) as no new version, with a link to the releases page", async () => {
    vi.mocked(check).mockRejectedValue("Could not fetch a valid release JSON from the remote");
    wrap(<Updates />);
    expect(await screen.findByText("Nenhuma versão nova pelo canal de atualizações automáticas.")).toBeInTheDocument();
    expect(screen.queryByText(/Could not fetch/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Não foi possível verificar/)).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Abrir página de versões" }));
    expect(openUrl).toHaveBeenCalledWith("https://github.com/Chamfrado/MasterOfNotes/releases");
  });
});
