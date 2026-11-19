// SPEC-004 T-004-05 (skeleton): device choice, live meter, noise calibration, latency offset — all via ipc.ts.
import { useEffect, useState } from "react";
import { MicMeter } from "../components/game";
import { Button, Card, ErrorState, LoadingState, Select, Slider, errorMessage, useToast } from "../components/ui";
import {
  calibrateNoise,
  getSettings,
  listInputDevices,
  startMicMonitor,
  stopMicMonitor,
  updateSettings,
  type AppSettings,
  type CalibrationIssue,
  type MicFrame,
  type NoiseCalibration,
} from "../lib/ipc";
import { useAsync } from "../useAsync";
import { LatencyProbe } from "./LatencyProbe";
import { LeakTest } from "./LeakTest";

const CALIBRATION_S = 3;
const issueText: Record<CalibrationIssue, string> = {
  noisy: "Ambiente barulhento: a pontuação pode ignorar trechos baixos.",
  tooQuiet: "Sinal muito baixo: aproxime o microfone ou aumente o ganho.",
  clipping: "O microfone está saturando: diminua o ganho de entrada.",
};

export function Mic() {
  const [load, reload] = useAsync(() => Promise.all([listInputDevices(), getSettings()]), []);
  return (
    <div className="flex flex-col gap-8">
      <header>
        <h1 className="font-display text-5xl font-bold">Microfone</h1>
        <p className="mt-2 text-muted">Escolha o microfone, confira o nível e calibre o ruído do ambiente.</p>
      </header>
      {load.status === "loading" && <LoadingState label="Procurando microfones…" />}
      {load.status === "error" && <ErrorState title="Não foi possível acessar o microfone" error={load.error} onRetry={reload} />}
      {load.status === "ready" && <MicSetup devices={load.data[0]} initial={load.data[1]} />}
    </div>
  );
}

function MicSetup({ devices, initial }: { devices: Awaited<ReturnType<typeof listInputDevices>>; initial: AppSettings }) {
  const toast = useToast();
  const [settings, setSettings] = useState(initial);
  const [frame, setFrame] = useState<MicFrame | null>(null);
  const [monitorError, setMonitorError] = useState<unknown>(null);
  const [calibrating, setCalibrating] = useState(false);
  const [verdict, setVerdict] = useState<NoiseCalibration | null>(null);
  const [monitorRun, setMonitorRun] = useState(0); // bump to restart the monitor after a leak test
  const deviceId = settings.inputDeviceId;

  useEffect(() => {
    setMonitorError(null);
    setFrame(null);
    startMicMonitor(deviceId, setFrame).catch(setMonitorError);
    return () => {
      stopMicMonitor().catch((e: unknown) => console.warn("stop_mic_monitor:", errorMessage(e)));
    };
  }, [deviceId, monitorRun]);

  // After a leak test or latency probe: Rust saved new settings; the run stopped the monitor.
  const afterRun = () => {
    getSettings().then(setSettings, (e: unknown) => toast(errorMessage(e), "error"));
    setMonitorRun((n) => n + 1);
  };

  const patch = async (p: Partial<AppSettings>) => {
    try {
      setSettings(await updateSettings(p));
    } catch (e) {
      toast(errorMessage(e), "error");
    }
  };

  const calibrate = async () => {
    setCalibrating(true);
    setVerdict(null);
    try {
      await stopMicMonitor(); // the calibration opens its own capture
      const result = await calibrateNoise(deviceId, CALIBRATION_S);
      setVerdict(result);
      setSettings((s) => ({ ...s, noiseFloorDbfs: result.noiseFloorDbfs }));
    } catch (e) {
      toast(errorMessage(e), "error");
    } finally {
      setCalibrating(false);
      startMicMonitor(deviceId, setFrame).catch(setMonitorError);
    }
  };

  return (
    <div className="grid gap-6 lg:grid-cols-2">
      <Card className="flex flex-col gap-6">
        <Select
          label="Dispositivo de entrada"
          value={deviceId ?? ""}
          onChange={(e) => patch({ inputDeviceId: e.currentTarget.value || null })}
          options={[
            { value: "", label: "Padrão do sistema" },
            ...devices.map((d) => ({ value: d.id, label: d.isDefault ? `${d.name} (padrão)` : d.name })),
          ]}
          hint={devices.length === 0 ? "Nenhum microfone encontrado. Conecte um e volte a esta tela." : undefined}
        />
        <div className="flex flex-col gap-2">
          <h2 className="font-semibold">Nível ao vivo</h2>
          <p className="text-muted">
            Cante uma nota: o selo “Voz detectada” deve acender e a nota aparecer. Use fones de ouvido — se o microfone
            captar a música durante a partida, avisamos na tela.
          </p>
          {monitorError ? (
            <p role="alert" className="text-danger">
              {errorMessage(monitorError)}
            </p>
          ) : (
            <MicMeter frame={frame} noiseFloorDbfs={settings.noiseFloorDbfs} />
          )}
        </div>
      </Card>

      <Card className="flex flex-col gap-6">
        <div className="flex flex-col items-start gap-3">
          <h2 className="font-semibold">Calibração de ruído</h2>
          <p className="text-muted">Fique em silêncio por {CALIBRATION_S} segundos enquanto medimos o ambiente.</p>
          <Button variant="primary" onClick={calibrate} disabled={calibrating}>
            {calibrating ? "Medindo… silêncio, por favor" : settings.noiseFloorDbfs == null ? "Calibrar" : "Calibrar de novo"}
          </Button>
          <div aria-live="polite">
            {verdict &&
              (verdict.issues.length === 0 ? (
                <p className="text-success">Ambiente bom para cantar.</p>
              ) : (
                <ul className="flex flex-col gap-1 text-warning">
                  {verdict.issues.map((i) => (
                    <li key={i}>{issueText[i]}</li>
                  ))}
                </ul>
              ))}
          </div>
        </div>
        <Slider
          label="Ajuste de latência"
          min={-500}
          max={500}
          step={10}
          value={settings.latencyOffsetMs}
          onChange={(v) => {
            setSettings((s) => ({ ...s, latencyOffsetMs: v }));
            void patch({ latencyOffsetMs: v });
          }}
          format={(v) => `${v > 0 ? "+" : ""}${v} ms`}
        />
        <Slider
          label="Volume da música"
          min={0}
          max={1}
          step={0.05}
          value={settings.videoVolume}
          onChange={(v) => {
            setSettings((s) => ({ ...s, videoVolume: v }));
            void patch({ videoVolume: v });
          }}
          format={(v) => `${Math.round(v * 100)}%`}
        />
        <p className="-mt-3 text-sm text-muted">
          Se a pontuação parece atrasada em relação ao que você canta, aumente; se parece adiantada, diminua.
        </p>
        <LatencyProbe onFinished={afterRun} />
      </Card>

      <Card className="lg:col-span-2">
        <LeakTest settings={settings} onFinished={afterRun} />
      </Card>
    </div>
  );
}
