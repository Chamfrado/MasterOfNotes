// SPEC-003 skeleton: <video> is the music clock; the reducer owns UI state; scoring lives in Rust.
// Full controls (seek/volume/practice mode, T-003-02) and sync hardening (T-003-03) build on this.
import { useEffect, useReducer, useRef, useState } from "react";
import { PitchIndicator, ScoreBadge } from "../components/game";
import { Button, ErrorState, LoadingState, errorMessage, useToast } from "../components/ui";
import {
  abortPerformance,
  finishPerformance,
  listSongs,
  startPerformance,
  syncPerformance,
  updateSong,
  videoSrc,
  type LiveTick,
  type PerformanceStarted,
  type Song,
} from "../lib/ipc";
import { useNav } from "../nav";
import { initialPlayerState, playerReducer } from "../player/playerMachine";
import { useAsync } from "../useAsync";

const COUNTDOWN_S = 3;
const SYNC_EVERY_MS = 250;
const CODEC_HINT =
  "Este vídeo não pode ser reproduzido (formato ou codec não suportado). Converta para MP4 com vídeo H.264 e áudio AAC.";

export function Player({ songId }: { songId: string }) {
  const [songs, reload] = useAsync(listSongs, []);
  if (songs.status === "loading") return <LoadingState label="Preparando o palco…" />;
  if (songs.status === "error") return <ErrorState error={songs.error} onRetry={reload} />;
  const song = songs.data.find((s) => s.id === songId);
  if (!song) return <ErrorState title="Música não encontrada" error="Ela pode ter sido removida da pasta da biblioteca." />;
  return <Stage song={song} />;
}

function Stage({ song }: { song: Song }) {
  const { navigate, back } = useNav();
  const toast = useToast();
  const video = useRef<HTMLVideoElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const [state, dispatch] = useReducer(playerReducer, initialPlayerState);
  const [perf, setPerf] = useState<PerformanceStarted | null>(null);
  const [tick, setTick] = useState<LiveTick | null>(null);
  const active = useRef(false); // a Rust performance is open and must be finished or aborted

  useEffect(() => {
    dispatch({ type: "LOAD" });
    return () => {
      if (active.current) abortPerformance().catch((e: unknown) => console.warn("abort_performance:", errorMessage(e)));
    };
  }, []);

  // Countdown 3-2-1 (R5), then the video starts.
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
        syncPerformance(meta.mediaTime, true, false).catch((e: unknown) => console.warn("sync_performance:", errorMessage(e)));
      }
      handle = v.requestVideoFrameCallback(onFrame);
    };
    handle = v.requestVideoFrameCallback(onFrame);
    return () => v.cancelVideoFrameCallback(handle);
  }, [state.status, perf]);

  const sync = (seeked: boolean) => {
    const v = video.current;
    if (!v || !active.current) return;
    syncPerformance(v.currentTime, !v.paused, seeked).catch((e: unknown) => console.warn("sync_performance:", errorMessage(e)));
  };

  const start = async () => {
    try {
      setPerf(await startPerformance({ songId: song.id, roundId: null, participantId: null }, setTick));
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

  // R2 shortcuts. Esc is handled globally (back), and unmount aborts the performance.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLButtonElement && e.key === " ") return; // let the focused button act
      if (e.key === " ") {
        e.preventDefault();
        togglePause();
      } else if (e.key === "f" || e.key === "F") {
        if (document.fullscreenElement) void document.exitFullscreen();
        else void stage.current?.requestFullscreen();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div ref={stage} className="relative flex h-dvh items-center justify-center overflow-hidden bg-black">
      <h1 className="sr-only">Tocando {song.title}</h1>
      <video
        ref={video}
        src={videoSrc(song)}
        preload="auto"
        className="size-full object-contain"
        onLoadedMetadata={(e) => {
          const durationMs = Math.round(e.currentTarget.duration * 1000);
          dispatch({ type: "LOADED", durationMs });
          updateSong(song.id, { durationMs, playable: "yes" }).catch((err: unknown) => console.warn("update_song:", errorMessage(err)));
        }}
        onError={() => {
          dispatch({ type: "FAIL", message: CODEC_HINT });
          updateSong(song.id, { playable: "no" }).catch((err: unknown) => console.warn("update_song:", errorMessage(err)));
        }}
        onPlay={() => {
          dispatch({ type: "RESUME" });
          sync(false);
        }}
        onPause={(e) => {
          if (!e.currentTarget.ended) dispatch({ type: "PAUSE" });
          sync(false);
        }}
        onSeeked={() => sync(true)}
        onEnded={finish}
      />

      {/* Feedback stays in the top band: the lower-center third is reserved for lyrics (R7). */}
      {perf && (state.status === "playing" || state.status === "paused") && (
        <div className="absolute inset-x-8 top-6 flex items-start justify-between gap-6">
          <div className="w-[min(28rem,50%)] rounded-lg bg-bg/70 p-4 backdrop-blur">
            <PitchIndicator feedback={tick?.feedback ?? null} />
          </div>
          <div className="rounded-lg bg-bg/70 px-6 py-3 backdrop-blur">
            <ScoreBadge score={tick?.feedback?.runningScore ?? 0} competitive={perf.competitive} />
          </div>
        </div>
      )}

      {state.status === "countdown" && (
        <p aria-live="assertive" className="absolute font-display text-[12rem] font-bold text-fg drop-shadow-lg">
          <span key={state.remaining} className="inline-block animate-enter">
            {state.remaining}
          </span>
        </p>
      )}

      {(state.status === "loading" || state.status === "ready" || state.status === "paused" || state.status === "error") && (
        <div className="absolute inset-0 grid place-items-center bg-bg/80 backdrop-blur-sm">
          <div className="flex max-w-xl flex-col items-center gap-6 text-center">
            <h2 className="font-display text-4xl font-bold">{song.title}</h2>
            {state.status === "loading" && <LoadingState label="Carregando vídeo…" />}
            {state.status === "ready" && (
              <Button variant="primary" size="lg" autoFocus onClick={start}>
                Começar
              </Button>
            )}
            {state.status === "paused" && (
              <>
                {perf && !perf.competitive && perf.reason && <p className="text-warning">{perf.reason}</p>}
                <Button variant="primary" size="lg" autoFocus onClick={togglePause}>
                  Continuar
                </Button>
              </>
            )}
            {state.status === "error" && <p role="alert" className="text-danger">{state.message}</p>}
            <Button variant="ghost" onClick={back}>
              Sair (Esc)
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
