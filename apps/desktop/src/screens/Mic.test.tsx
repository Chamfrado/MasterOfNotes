import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "../components/ui";
import * as ipc from "../lib/ipc";
import { Mic } from "./Mic";

vi.mock("../lib/ipc", () => ({
  listInputDevices: vi.fn(),
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
  startMicMonitor: vi.fn(),
  stopMicMonitor: vi.fn(() => Promise.resolve()),
  calibrateNoise: vi.fn(),
  listSongs: vi.fn(() => Promise.resolve([])),
  getReference: vi.fn(),
  startPerformance: vi.fn(),
  syncPerformance: vi.fn(),
  finishPerformance: vi.fn(),
  abortPerformance: vi.fn(),
  videoSrc: () => "asset://v.mp4",
}));

const settings = { inputDeviceId: null, noiseFloorDbfs: null, latencyOffsetMs: 0 } as ipc.AppSettings;
const renderMic = () =>
  render(
    <ToastProvider>
      <Mic />
    </ToastProvider>,
  );

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(ipc.listInputDevices).mockResolvedValue([
    { id: "usb", name: "USB Mic", isDefault: false, sampleRate: 48000, channels: 1 },
    { id: "int", name: "Interno", isDefault: true, sampleRate: 48000, channels: 2 },
  ]);
  vi.mocked(ipc.getSettings).mockResolvedValue(settings);
  vi.mocked(ipc.startMicMonitor).mockResolvedValue(undefined);
});

describe("Mic", () => {
  it("lists devices, persists the choice and restarts the monitor on it", async () => {
    vi.mocked(ipc.updateSettings).mockResolvedValue({ ...settings, inputDeviceId: "usb" });
    renderMic();
    const select = await screen.findByRole("combobox", { name: "Dispositivo de entrada" });
    expect(screen.getByRole("option", { name: "Interno (padrão)" })).toBeInTheDocument();
    await vi.waitFor(() => expect(ipc.startMicMonitor).toHaveBeenCalledWith(null, expect.any(Function)));
    await userEvent.selectOptions(select, "usb");
    expect(ipc.updateSettings).toHaveBeenCalledWith({ inputDeviceId: "usb" });
    expect(ipc.stopMicMonitor).toHaveBeenCalled();
    await vi.waitFor(() => expect(ipc.startMicMonitor).toHaveBeenLastCalledWith("usb", expect.any(Function)));
  });

  it("drives the meter from monitor frames", async () => {
    renderMic();
    await screen.findByRole("meter");
    const onFrame = vi.mocked(ipc.startMicMonitor).mock.calls[0][1];
    act(() => onFrame({ rmsDbfs: -18, f0Hz: 440, confidence: 0.9, voiced: true, clipped: false }));
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuenow", "-18");
    expect(screen.getByText("Nota A4 · 440 Hz")).toBeInTheDocument();
  });

  it("shows the real monitor error", async () => {
    vi.mocked(ipc.startMicMonitor).mockRejectedValue({ code: "audio", message: "Microfone em uso por outro programa." });
    renderMic();
    expect(await screen.findByText("Microfone em uso por outro programa.")).toBeInTheDocument();
  });

  it("calibrates and lists each issue, or confirms a good room", async () => {
    vi.mocked(ipc.calibrateNoise).mockResolvedValueOnce({ noiseFloorDbfs: -35, clippedRatio: 0, issues: ["noisy", "clipping"] });
    renderMic();
    await userEvent.click(await screen.findByRole("button", { name: "Calibrar" }));
    expect(ipc.calibrateNoise).toHaveBeenCalledWith(null, 3);
    expect(await screen.findByText(/Ambiente barulhento/)).toBeInTheDocument();
    expect(screen.getByText(/saturando: diminua o ganho/)).toBeInTheDocument();

    vi.mocked(ipc.calibrateNoise).mockResolvedValueOnce({ noiseFloorDbfs: -60, clippedRatio: 0, issues: [] });
    await userEvent.click(screen.getByRole("button", { name: "Calibrar de novo" }));
    expect(await screen.findByText("Ambiente bom para cantar.")).toBeInTheDocument();
  });

  it("persists the latency offset", async () => {
    vi.mocked(ipc.updateSettings).mockResolvedValue({ ...settings, latencyOffsetMs: 10 });
    renderMic();
    const slider = await screen.findByRole("slider", { name: "Ajuste de latência" });
    expect(slider).toHaveAttribute("aria-valuetext", "0 ms");
    fireEvent.change(slider, { target: { value: "10" } });
    expect(slider).toHaveAttribute("aria-valuetext", "+10 ms");
    expect(ipc.updateSettings).toHaveBeenCalledWith({ latencyOffsetMs: 10 });
  });
});
