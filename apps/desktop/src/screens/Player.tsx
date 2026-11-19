// SPEC-003 Player: <video> is the music clock; the reducer owns UI state; scoring lives in Rust.
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useReducer, useRef, useState, type ReactNode } from "react";
import { PitchIndicator, ScoreBadge } from "../components/game";
import { Badge, Button, Dialog, ErrorState, LoadingState, errorMessage, useToast } from "../components/ui";
import {
  abortPerformance,
  finishPerformance,
  getSettings,
  listSongs,
  startPerformance,
  syncPerformance,
  updateSettings,
  updateSong,
  videoSrc,
  type AppSettings,
  type LiveTick,
  type PerformanceStarted,
  type Song,
} from "../lib/ipc";
import { useNav } from "../nav";
import { createLeakMonitor } from "../player/leak";
import { initialPlayerState, playerReducer } from "../player/playerMachine";
import { useAsync } from "../useAsync";

const COUNTDOWN_S = 3;
const SYNC_EVERY_MS = 250;
const SEEK_STEP_S = 5;
const VOLUME_STEP = 0.05;
const CONTROLS_HIDE_MS = 3000;
const CODEC_HINT =
  "Este vídeo não pode ser reproduzido (formato ou codec não suportado). Converta para MP4 com vídeo H.264 e áudio AAC.";

type Overlay = AppSettings["feedbackOverlay"];
const overlayModes: Overlay[] = ["full", "compact", "hidden"];
const overlayText: Record<Overlay, string> = { full: "completo", compact: "compacto", hidden: "oculto" };

/** Seconds → "m:ss". */
export const clock = (s: number) => {
  const t = Math.max(0, Math.floor(Number.isFinite(s) ? s : 0));
  return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, "0")}`;
};

const warn = (what: string) => (e: unknown) => console.warn(`${what}:`, errorMessage(e));

export function Player({ songId }: { songId: string }) {
  const [load, reload] = useAsync(() => Promise.all([listSongs(), getSettings()]), []);
  if (load.status === "loading")
    return (
      <Backstage>
        <LoadingState label="Preparando o palco…" />
      </Backstage>
    );
  if (load.status === "error")
    return (
      <Backstage>
        <ErrorState error={load.error} onRetry={reload} />
      </Backstage>
    );
  const [songs, settings] = load.data;
  const song = songs.find((s) => s.id === songId);
  if (!song)
    return (
      <Backstage>
        <ErrorState title="Música não encontrada" error="Ela pode ter sido removida da pasta da biblioteca." />
      </Backstage>
    );
  return <Stage song={song} settings={settings} />;
}

/** The player hides the NavBar, so pre-stage states keep an explicit way out. */
function Backstage({ children }: { children: ReactNode }) {
  const { back } = useNav();
  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6 p-8">
      <Button variant="ghost" className="self-start" onClick={back}>
        ← Voltar (Esc)
      </Button>
      {children}
    </div>
  );
}

const toggleFullscreen = async () => {
  const w = getCurrentWindow();
  await w.setFullscreen(!(await w.isFullscreen()));
};

function Stage({ song, settings }: { song: Song; settings: AppSettings }) {
  const { navigate, back } = useNav();
  const toast = useToast();
  const video = useRef<HTMLVideoElement>(null);
  const [state, dispatch] = useReducer(playerReducer, initialPlayerState);
  const [perf, setPerf] = useState<PerformanceStarted | null>(null);
  const [tick, setTick] = useState<LiveTick | null>(null);
  const [time, setTime] = useState(0);
  const [volume, setVolume] = useState(settings.videoVolume);
  const [overlay, setOverlay] = useState<Overlay>(settings.feedbackOverlay);
  const [seekedAway, setSeekedAway] = useState(false); // a competitive run turned into practice by a seek
  const [pendingSeek, setPendingSeek] = useState<number | null>(null); // seek waiting for confirmation
  const [controls, setControls] = useState(true);
  const active = useRef(false); // a Rust performance is open and must be finished or aborted
  const leakMonitor = useRef(createLeakMonitor());
  const [leak, setLeak] = useState(false);
  const practice = seekedAway || (perf != null && !perf.competitive);

  useEffect(() => {
    dispatch({ type: "LOAD" });
    return () => {
      if (active.current) abortPerformance().catch(warn("abort_performance"));
      // Leaving the stage always restores the windowed app.
      getCurrentWindow().setFullscreen(false).catch(warn("setFullscreen"));
    };
  }, []);

  // R5: countdown 3-2-1, then the video starts.
  useEffect(() => {
    if (state.status !== "countdown") return;
    const t = setTimeout(() => dispatch({ type: "TICK" }), 1000);
    return () => clearTimeout(t);
  }, [state]);
  useEffect(() => {
    if (state.status === "playing" && video.current?.paused) {
      video.current.play().catch((e: unknown) => dispatch({ type: "FAIL", message: errorMessage(e) }));
    }
  }, [state.status]);

  // R6: periodic sync from the video's own media clock, never the UI clock.
  useEffect(() => {
    const v = video.current;
    if (state.status !== "playing" || !v || !perf) return;
    let handle = 0;
    let last = 0;
    const onFrame: VideoFrameRequestCallback = (now, meta) => {
      if (now - last >= SYNC_EVERY_MS) {
        last = now;
        syncPerformance(meta.mediaTime, true, false).catch(warn("sync_performance"));
      }
      handle = v.requestVideoFrameCallback(onFrame);
    };
    handle = v.requestVideoFrameCallback(onFrame);
    return () => v.cancelVideoFrameCallback(handle);
  }, [state.status, perf]);

  // Controls fade out while singing; pointer or key activity brings them back.
  useEffect(() => {
    if (!controls || state.status !== "playing") return;
    const t = setTimeout(() => setControls(false), CONTROLS_HIDE_MS);
    return () => clearTimeout(t);
  }, [controls, state.status]);

  const sync = (playing: boolean, seeked: boolean) => {
    const v = video.current;
    if (v && active.current) syncPerformance(v.currentTime, playing, seeked).catch(warn("sync_performance"));
  };

  const start = async () => {
    try {
      const onTick = (t: LiveTick) => {
        setTick(t);
        setLeak(leakMonitor.current(t)); // same value = no re-render
      };
      setPerf(await startPerformance({ songId: song.id, roundId: null, participantId: null }, onTick));
      active.current = true;
      dispatch({ type: "START", countdown: COUNTDOWN_S });
    } catch (e) {
      toast(errorMessage(e), "error");
    }
  };

  const finish = async () => {
    dispatch({ type: "ENDED" });
    if (!active.current) return;
    active.current = false;
    try {
      navigate({ name: "result", songId: song.id, outcome: await finishPerformance() });
    } catch (e) {
      dispatch({ type: "FAIL", message: errorMessage(e) });
    }
  };

  const togglePause = () => {
    const v = video.current;
    if (!v) return;
    if (state.status === "playing") v.pause();
    else if (state.status === "paused") v.play().catch((e: unknown) => dispatch({ type: "FAIL", message: errorMessage(e) }));
  };

  const live = state.status === "playing" || state.status === "paused";
  /** R2: seeking a competitive run turns it into practice, so ask first. */
  const requestSeek = (to: number) => {
    const v = video.current;
    if (!v || !live) return;
    const target = Math.max(0, Math.min(v.duration || 0, to));
    if (practice) v.currentTime = target;
    else setPendingSeek(target);
  };
  const confirmSeek = () => {
    if (pendingSeek != null && video.current) {
      setSeekedAway(true);
      video.current.currentTime = pendingSeek;
    }
    setPendingSeek(null);
  };

  const changeVolume = (value: number) => {
    const next = Math.round(Math.max(0, Math.min(1, value)) * 100) / 100;
    setVolume(next);
    if (video.current) video.current.volume = next;
    updateSettings({ videoVolume: next }).catch(warn("update_settings"));
  };

  const cycleOverlay = () => {
    const next = overlayModes[(overlayModes.indexOf(overlay) + 1) % overlayModes.length];
    setOverlay(next);
    toast(`Painel de feedback: ${overlayText[next]}`);
    updateSettings({ feedbackOverlay: next }).catch(warn("update_settings"));
  };

  const fullscreen = () => toggleFullscreen().catch((e: unknown) => toast(errorMessage(e), "error"));

  // R2 shortcuts. Esc is handled globally (back); unmount aborts the performance.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      setControls(true);
      if (pendingSeek != null) return; // the confirmation dialog owns the keyboard
      const t = e.target;
      if (t instanceof HTMLInputElement) return; // sliders handle their own arrows
      if (t instanceof HTMLButtonElement && (e.key === " " || e.key === "Enter")) return;
      const v = video.current;
      switch (e.key) {
        case " ":
          togglePause();
          break;
        case "f":
        case "F":
          fullscreen();
          break;
        case "h":
        case "H":
          cycleOverlay();
          break;
        case "ArrowUp":
          changeVolume(volume + VOLUME_STEP);
          break;
        case "ArrowDown":
          changeVolume(volume - VOLUME_STEP);
          break;
        case "ArrowLeft":
          if (v) requestSeek(v.currentTime - SEEK_STEP_S);
          break;
        case "ArrowRight":
          if (v) requestSeek(v.currentTime + SEEK_STEP_S);
          break;
        default:
          return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const durationS = "durationMs" in state ? state.durationMs / 1000 : 0;
  const showControls = controls || state.status !== "playing";

  return (
    <div
      onPointerMove={() => setControls(true)}
      className={`relative flex h-dvh items-center justify-center overflow-hidden bg-black ${showControls ? "" : "cursor-none"}`}
    >
      <h1 className="sr-only">Tocando {song.title}</h1>
      <video
        ref={video}
        src={videoSrc(song)}
        preload="auto"
        className="size-full object-contain"
        onLoadedMetadata={(e) => {
          e.currentTarget.volume = volume;
          const durationMs = Math.round(e.currentTarget.duration * 1000);
          dispatch({ type: "LOADED", durationMs });
          updateSong(song.id, { durationMs, playable: "yes" }).catch(warn("update_song"));
        }}
        onError={() => {
          dispatch({ type: "FAIL", message: CODEC_HINT });
          updateSong(song.id, { playable: "no" }).catch(warn("update_song"));
        }}
        onTimeUpdate={(e) => setTime(e.currentTarget.currentTime)}
        onPlay={() => {
          dispatch({ type: "RESUME" });
          sync(true, false);
        }}
        onPause={(e) => {
          if (!e.currentTarget.ended) dispatch({ type: "PAUSE" });
          sync(false, false);
        }}
        // R6/R8: frames are dropped while seeking; seeked re-anchors (Rust marks the run as practice).
        onSeeking={() => sync(false, true)}
        onSeeked={(e) => sync(!e.currentTarget.paused, true)}
        onEnded={finish}
      />

      {/* R7: feedback stays in the top band; the lower-center third is reserved for lyrics. */}
      {perf && overlay !== "hidden" && live && (
        <div className="pointer-events-none absolute inset-x-8 top-6 flex items-start justify-between gap-6">
          {overlay === "full" ? (
            <div className="w-[min(28rem,50%)] rounded-lg bg-bg/70 p-4 backdrop-blur">
              <PitchIndicator feedback={tick?.feedback ?? null} />
            </div>
          ) : (
            <span />
          )}
          <div className="rounded-lg bg-bg/70 px-6 py-3 backdrop-blur">
            <ScoreBadge score={tick?.feedback?.runningScore ?? 0} competitive={!practice} />
          </div>
        </div>
      )}

      {/* SPEC-004 R9 */}
      <div role="status" className="pointer-events-none absolute top-28 left-1/2 -translate-x-1/2">
        {leak && live && (
          <p className="rounded-md border border-warning/50 bg-bg/85 px-5 py-3 font-semibold text-warning">
            O microfone está captando a música — use fones de ouvido.
          </p>
        )}
      </div>

      {state.status === "countdown" && (
        <p aria-live="assertive" className="absolute font-display text-[12rem] font-bold text-fg drop-shadow-lg">
          <span key={state.remaining} className="inline-block animate-enter">
            {state.remaining}
          </span>
        </p>
      )}

      {live && (
        <div
          className={`absolute inset-x-0 bottom-0 flex flex-col gap-3 bg-linear-to-t from-bg/90 to-transparent px-8 pt-16 pb-6 transition-opacity duration-(--duration-base) ${showControls ? "opacity-100" : "pointer-events-none opacity-0"}`}
        >
          {state.status === "paused" && perf && !perf.competitive && perf.reason && (
            <p className="self-start rounded-md bg-bg/80 px-4 py-2 text-warning">{perf.reason}</p>
          )}
          <div className="flex items-center gap-4 tabular-nums">
            <span className="text-sm">{clock(time)}</span>
            <input
              type="range"
              aria-label="Posição na música"
              aria-valuetext={`${clock(time)} de ${clock(durationS)}`}
              min={0}
              max={durationS}
              step={1}
              value={Math.min(time, durationS)}
              onChange={(e) => requestSeek(Number(e.currentTarget.value))}
              className="h-12 flex-1 cursor-pointer"
            />
            <span className="text-sm text-muted">{clock(durationS)}</span>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="primary" onClick={togglePause} autoFocus={state.status === "paused"}>
              {state.status === "playing" ? "Pausar" : "Continuar"}
            </Button>
            <label className="flex items-center gap-3 px-2">
              <span className="text-sm text-muted">Volume</span>
              <input
                type="range"
                min={0}
                max={1}
                step={VOLUME_STEP}
                value={volume}
                aria-valuetext={`${Math.round(volume * 100)}%`}
                onChange={(e) => changeVolume(Number(e.currentTarget.value))}
                className="h-12 w-32 cursor-pointer"
              />
            </label>
            {practice && <Badge tone="warning">Treino — fora do ranking</Badge>}
            <span className="flex-1" />
            <Button variant="ghost" onClick={cycleOverlay}>
              Painel: {overlayText[overlay]} (H)
            </Button>
            <Button variant="ghost" onClick={fullscreen}>
              Tela cheia (F)
            </Button>
            <Button variant="ghost" onClick={back}>
              Sair (Esc)
            </Button>
          </div>
        </div>
      )}

      {(state.status === "loading" || state.status === "ready" || state.status === "error") && (
        <div className="absolute inset-0 grid place-items-center bg-bg/80 backdrop-blur-sm">
          <div className="flex max-w-xl flex-col items-center gap-6 text-center">
            <h2 className="font-display text-4xl font-bold">{song.title}</h2>
            {state.status === "loading" && <LoadingState label="Carregando vídeo…" />}
            {state.status === "ready" && (
              <Button variant="primary" size="lg" autoFocus onClick={start}>
                Começar
              </Button>
            )}
            {state.status === "error" && (
              <p role="alert" className="text-danger">
                {state.message}
              </p>
            )}
            <Button variant="ghost" onClick={back}>
              Sair (Esc)
            </Button>
          </div>
        </div>
      )}

      <Dialog open={pendingSeek != null} onClose={() => setPendingSeek(null)} title="Pular trecho?">
        <p className="mb-6 text-muted">
          Avançar ou voltar a música transforma esta partida em <strong className="text-fg">treino</strong>: a nota não entra
          no ranking.
        </p>
        <div className="flex justify-end gap-3">
          <Button variant="ghost" onClick={() => setPendingSeek(null)}>
            Continuar valendo
          </Button>
          <Button variant="primary" onClick={confirmSeek}>
            Pular e virar treino
          </Button>
        </div>
      </Dialog>
    </div>
  );
}
