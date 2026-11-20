// T-004-07 (SPEC-004 R7): assisted latency measure. Plays the click track through a media element,
// the user claps on each click, Rust pairs mic onsets with the click times and saves latencyOffsetMs if it passes.
import { useEffect, useRef, useState } from "react";
import { Button, Progress, errorMessage } from "../components/ui";
import { abortPerformance, finishPerformance, listSongs, startPerformance, stopMicMonitor, syncPerformance, type LatencyVerdict } from "../lib/ipc";
import { CLICK_COUNT, clickTrack } from "../player/clickTrack";
import { SYNC_EVERY_MS, warn } from "../player/sync";

type Phase = { name: "idle" } | { name: "running" } | { name: "done"; verdict: LatencyVerdict } | { name: "error"; error: unknown };

const ms = (n: number) => `${Math.round(n)} ms`;
const JITTER_REPEAT_MS = 10; // SPEC-006 N4: above this, recommend measuring again

/** `onFinished` reloads settings (a passed probe updated latencyOffsetMs) and restarts the mic monitor. */
export function LatencyProbe({ onFinished }: { onFinished: () => void }) {
  const [phase, setPhase] = useState<Phase>({ name: "idle" });
  const audio = useRef<HTMLAudioElement>(null);
  const open = useRef(false);

  useEffect(
    () => () => {
      if (open.current) abortPerformance().catch(warn("abort_performance"));
    },
    [],
  );

  const run = async () => {
    const a = audio.current;
    if (!a) return;
    setPhase({ name: "running" });
    const track = clickTrack();
    const url = URL.createObjectURL(new Blob([track.wav], { type: "audio/wav" }));
    let timer = 0;
    const sync = (playing: boolean) => syncPerformance(a.currentTime, playing, false).catch(warn("sync_performance"));
    try {
      // ❓ start_performance still requires an existing songId (Agent 1 note); any library song works.
      const song = (await listSongs()).find((s) => s.available);
      if (!song) throw new Error("Adicione ao menos uma música à biblioteca para medir a latência.");
      await stopMicMonitor();
      a.src = url;
      await startPerformance({ songId: song.id, roundId: null, participantId: null, latencyProbe: track.clicksMs }, () => {});
      open.current = true;
      const ended = new Promise<void>((ok, fail) => {
        a.onended = () => ok();
        a.onerror = () => fail(new Error("Não foi possível tocar os cliques."));
      });
      a.onplaying = () => sync(true); // anchor at the media clock
      await a.play();
      timer = window.setInterval(() => sync(true), SYNC_EVERY_MS);
      await ended;
      window.clearInterval(timer);
      await sync(false);
      open.current = false;
      const outcome = await finishPerformance();
      if (!outcome.latency) throw new Error("A medição terminou sem resultado. Tente de novo.");
      setPhase({ name: "done", verdict: outcome.latency });
    } catch (e) {
      window.clearInterval(timer);
      a.pause();
      if (open.current) {
        open.current = false;
        abortPerformance().catch(warn("abort_performance"));
      }
      setPhase({ name: "error", error: e });
    } finally {
      URL.revokeObjectURL(url);
      onFinished();
    }
  };

  const running = phase.name === "running";
  return (
    <div className="flex flex-col gap-3">
      <audio ref={audio} preload="auto" className="hidden" />
      <h3 className="font-semibold">Medir automaticamente</h3>
      <p className="text-muted">
        Vão tocar {CLICK_COUNT} cliques. Bata uma palma junto com cada clique, perto do microfone.
      </p>
      <div aria-live="polite">
        {running && (
          <div className="flex flex-col gap-2">
            <p className="font-semibold">Palmas no ritmo dos cliques…</p>
            <Progress label="Medindo latência" />
          </div>
        )}
        {phase.name === "done" &&
          (phase.verdict.passed ? (
            <>
              <p className="text-success">
                Latência medida: {ms(phase.verdict.offsetMs)} (variação ±{ms(phase.verdict.jitterMs)}, {phase.verdict.detected} de{" "}
                {phase.verdict.expected} palmas). Ajuste salvo.
              </p>
              {/* SPEC-006 N4: compare what is shown, so "±10 ms" never comes with "above 10 ms". */}
              {Math.round(phase.verdict.jitterMs) > JITTER_REPEAT_MS && (
                <p className="text-warning">
                  A variação passou de {JITTER_REPEAT_MS} ms, então a medida é pouco precisa. Recomendamos medir de novo, com palmas
                  secas e o ambiente em silêncio.
                </p>
              )}
            </>
          ) : (
            <p className="text-danger">
              {phase.verdict.reason ?? "Não deu para medir."} O ajuste atual foi mantido.
            </p>
          ))}
        {phase.name === "error" && <p className="text-danger">{errorMessage(phase.error)}</p>}
      </div>
      <Button className="self-start" disabled={running} onClick={run}>
        {running ? "Medindo…" : phase.name === "idle" ? "Medir latência" : "Medir de novo"}
      </Button>
    </div>
  );
}
