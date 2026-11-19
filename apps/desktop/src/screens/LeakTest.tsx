// T-004-06: leak check. Plays ~10 s of a song with a reference, from its first note, at the current
// volume while the user stays silent; Rust decides and saves settings.leakCheck.
import { useEffect, useRef, useState } from "react";
import { Badge, Button, Progress, errorMessage, type Tone } from "../components/ui";
import {
  abortPerformance,
  finishPerformance,
  getReference,
  listSongs,
  startPerformance,
  stopMicMonitor,
  syncPerformance,
  videoSrc,
  type AppSettings,
  type Song,
} from "../lib/ipc";
import { leakCheckState, type LeakCheckState } from "../player/leak";
import { startVideoSync, warn } from "../player/sync";
import { useAsync } from "../useAsync";

export const LEAK_TEST_S = 10;

/** Prefer a validated reference, then experimental; the file must be playable. */
export function pickLeakSong(songs: Song[]): Song | null {
  const ok = songs.filter((s) => s.available && s.playable !== "no");
  return ok.find((s) => s.referenceStatus === "validated") ?? ok.find((s) => s.referenceStatus === "experimental") ?? null;
}

const stateText: Record<LeakCheckState, { tone: Tone; text: string }> = {
  passed: { tone: "success", text: "Aprovado" },
  failed: { tone: "danger", text: "Reprovado" },
  stale: { tone: "warning", text: "Refazer (microfone ou volume mudou)" },
  none: { tone: "neutral", text: "Não feito" },
};

type Phase =
  | { name: "idle" }
  | { name: "running"; left: number }
  | { name: "done"; passed: boolean; reason: string | null }
  | { name: "error"; error: unknown };

/** `onFinished` restarts the mic monitor and reloads settings (Rust saved the new leakCheck). */
export function LeakTest({ settings, onFinished }: { settings: AppSettings; onFinished: () => void }) {
  const [songs] = useAsync(listSongs, []);
  const [phase, setPhase] = useState<Phase>({ name: "idle" });
  const video = useRef<HTMLVideoElement>(null);
  const open = useRef(false); // a leak-check performance is open in Rust
  const song = songs.status === "ready" ? pickLeakSong(songs.data) : null;
  const state = leakCheckState(settings);

  useEffect(
    () => () => {
      if (open.current) abortPerformance().catch(warn("abort_performance"));
    },
    [],
  );

  const run = async () => {
    const v = video.current;
    if (!song || !v) return;
    let stopSync = () => {};
    try {
      const ref = await getReference(song.id);
      if (!ref || ref.notes.length === 0) throw new Error("A referência desta música não tem notas.");
      await stopMicMonitor(); // the run opens its own capture
      if (v.readyState < HTMLMediaElement.HAVE_METADATA) {
        await new Promise<void>((ok, fail) => {
          v.addEventListener("loadedmetadata", () => ok(), { once: true });
          v.addEventListener("error", () => fail(new Error("Não foi possível abrir o vídeo da música do teste.")), { once: true });
        });
      }
      v.volume = settings.videoVolume;
      v.currentTime = Math.max(0, (ref.notes[0].startMs + ref.offsetMs) / 1000);
      await startPerformance({ songId: song.id, roundId: null, participantId: null, leakCheck: true }, () => {});
      open.current = true;
      await v.play();
      syncPerformance(v.currentTime, true, false).catch(warn("sync_performance"));
      stopSync = startVideoSync(v);
      for (let left = LEAK_TEST_S; left > 0; left--) {
        setPhase({ name: "running", left });
        await new Promise((r) => setTimeout(r, 1000));
      }
      stopSync();
      v.pause();
      await syncPerformance(v.currentTime, false, false);
      open.current = false;
      const outcome = await finishPerformance();
      if (!outcome.leak) throw new Error("O teste terminou sem resultado. Tente de novo.");
      setPhase({ name: "done", passed: outcome.leak.passed, reason: outcome.leak.reason });
    } catch (e) {
      stopSync();
      v.pause();
      if (open.current) {
        open.current = false;
        abortPerformance().catch(warn("abort_performance"));
      }
      setPhase({ name: "error", error: e });
    } finally {
      onFinished();
    }
  };

  const running = phase.name === "running";
  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-3">
        <h2 className="font-semibold">Teste de vazamento</h2>
        <Badge tone={stateText[state].tone}>{stateText[state].text}</Badge>
      </div>
      <p className="text-muted">
        Para valer ranking, o microfone não pode captar a música. Tocamos {LEAK_TEST_S} segundos no volume atual (
        {Math.round(settings.videoVolume * 100)}%): fique em silêncio.
      </p>
      {songs.status === "ready" && !song && (
        <p className="text-warning">Importe uma referência melódica em alguma música da biblioteca para fazer o teste.</p>
      )}
      {songs.status === "error" && <p className="text-danger">{errorMessage(songs.error)}</p>}
      {song && (
        <video ref={video} src={videoSrc(song)} preload="auto" className={running ? "aspect-video w-full rounded-md" : "hidden"} />
      )}
      <div aria-live="polite" className="flex flex-col gap-2">
        {running && (
          <>
            <p className="font-semibold">Silêncio… {phase.left} s</p>
            <Progress label="Teste de vazamento" value={LEAK_TEST_S - phase.left} max={LEAK_TEST_S} />
          </>
        )}
        {phase.name === "done" &&
          (phase.passed ? (
            <p className="text-success">Aprovado: partidas com este microfone valem ranking até este volume.</p>
          ) : (
            <div className="text-danger">
              {/* Rust's reason already says what to do; the generic hint is only for when it has none. */}
              <p>{phase.reason ?? "O microfone captou a música. Use fones de ouvido ou baixe o volume e repita."}</p>
            </div>
          ))}
        {phase.name === "error" && <p className="text-danger">{errorMessage(phase.error)}</p>}
      </div>
      <Button variant={state === "passed" ? "secondary" : "primary"} className="self-start" disabled={!song || running} onClick={run}>
        {running ? "Testando…" : state === "none" ? "Fazer teste" : "Testar de novo"}
      </Button>
      {song && <p className="text-sm text-muted">Música do teste: {song.title}</p>}
    </div>
  );
}
