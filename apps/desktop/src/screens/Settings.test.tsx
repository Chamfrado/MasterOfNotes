import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components/ui";
import * as ipc from "../lib/ipc";
import { NavContext } from "../nav";
import { Home } from "./Home";
import { Settings } from "./Settings";
import { Updates } from "./Updates";

vi.mock("../lib/ipc", () => ({ getSettings: vi.fn(), updateSettings: vi.fn() }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: () => Promise.resolve("0.1.0") }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
import { check, type Update } from "@tauri-apps/plugin-updater";

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
} as ipc.AppSettings;
const nav = { route: { name: "settings" as const }, navigate: vi.fn(), replace: vi.fn(), back: vi.fn() };
const wrap = (ui: React.ReactNode) =>
  render(
    <NavContext.Provider value={nav}>
      <ToastProvider>{ui}</ToastProvider>
    </NavContext.Provider>,
  );

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  delete document.documentElement.dataset.tv;
  delete document.documentElement.dataset.reducedMotion;
  vi.mocked(ipc.getSettings).mockResolvedValue(base);
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
    expect(localStorage.getItem("mon.tvMode")).toBe("1");
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
    vi.mocked(ipc.updateSettings).mockResolvedValue({ ...base, autoCheckUpdates: true });
    const { unmount } = wrap(<Home />);
    await userEvent.click(await screen.findByRole("button", { name: "Sim, verificar" }));
    expect(ipc.updateSettings).toHaveBeenCalledWith({ autoCheckUpdates: true });
    expect(screen.queryByRole("region", { name: "Atualizações automáticas" })).not.toBeInTheDocument();
    unmount();
    wrap(<Home />);
    await screen.findByRole("heading", { name: "Preparação" });
    expect(screen.queryByRole("region", { name: "Atualizações automáticas" })).not.toBeInTheDocument();
  });
  it("'Agora não' keeps it off and does not ask again", async () => {
    wrap(<Home />);
    await userEvent.click(await screen.findByRole("button", { name: "Agora não" }));
    expect(ipc.updateSettings).not.toHaveBeenCalled();
    expect(localStorage.getItem("mon.updateConsentAsked")).toBe("1");
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

  it("explains failures (no key / network) and keeps the app usable", async () => {
    vi.mocked(check).mockRejectedValue("Could not fetch a valid release JSON from the remote");
    wrap(<Updates />);
    expect(await screen.findByText(/O app continua funcionando normalmente\./)).toBeInTheDocument();
    expect(screen.getByText(/Could not fetch a valid release JSON/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Tentar de novo" })).toBeInTheDocument();
  });
});
