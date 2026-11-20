// SPEC-005 T-005-04: import (.mid/.midi/.json), pick the MIDI track, align the offset over the video, validate.
// T-005-07 (R8, ADR-0004 A): or sing the melody over the video; Rust keeps only the detected notes.
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { MicMeter } from "../components/game";
import { Badge, Button, Card, Dialog, ErrorState, LoadingState, Slider, errorMessage, useToast } from "../components/ui";
import {
  abortPerformance,
  finishPerformance,
  getReference,
  importReference,
  listMidiTracks,
  listSongs,
  setReferenceOffset,
  setReferenceStatus,
  startPerformance,
  syncPerformance,
  videoSrc,
  type MelodyReference,
  type MicFrame,
  type MidiTrackInfo,
  type RefNote,
  type Song,
} from "../lib/ipc";
import { useNav } from "../nav";
import { useAsync } from "../useAsync";
import { startVideoSync, warn } from "../player/sync";
import { referenceLabel } from "./Library";

const OFFSET_LIMIT_MS = 10_000; // R4
const ROLL_BEFORE_MS = 2_000;
const ROLL_AFTER_MS = 4_000;
const COUNTDOWN_S = 3;
const sourceLabel: Record<MelodyReference["source"], string> = { midi: "MIDI", json: "JSON", manual: "Cantada", extracted: "Extraída" };

export interface RollBox {
  x: number; // 0..1, left edge
  w: number; // 0..1
  y: number; // 0..1, top edge (high notes on top)
  h: number;
  active: boolean; // under the playhead
}

/** Piano-roll geometry for the window [now - before, now + after]; positive offset delays notes (start + offset). */
export function rollLayout(notes: RefNote[], offsetMs: number, nowMs: number, beforeMs = ROLL_BEFORE_MS, afterMs = ROLL_AFTER_MS): RollBox[] {
  if (notes.length === 0) return [];
  let lo = Infinity;
  let hi = -Infinity;
  for (const n of notes) {
    lo = Math.min(lo, n.midi);
    hi = Math.max(hi, n.midi);
  }
  lo -= 2;
  hi += 2;
  const rows = hi - lo;
  const from = nowMs - beforeMs;
  const span = beforeMs + afterMs;
  const out: RollBox[] = [];
  for (const n of notes) {
    const start = n.startMs + offsetMs;
    const end = start + n.durMs;
    if (end < from || start > from + span) continue;
    out.push({
      x: (start - from) / span,
      w: n.durMs / span,
      y: (hi - n.midi - 0.5) / rows,
      h: 1 / rows,
      active: start <= nowMs && nowMs < end,
    });
  }
  return out;
}

export function Reference({ songId }: { songId: string }) {
  const [load, reload] = useAsync(() => Promise.all([listSongs(), getReference(songId)]), [songId]);
  return (
    <div className="flex flex-col gap-8">
      {load.status === "loading" && <LoadingState label="Carregando referência…" />}
      {load.status === "error" && <ErrorState title="Não foi possível carregar a referência" error={load.error} onRetry={reload} />}
      {load.status === "ready" &&
        (() => {
          const song = load.data[0].find((s) => s.id === songId);
          return song ? (
            <Editor song={song} initial={load.data[1]} />
          ) : (
            <ErrorState title="Música não encontrada" error="Ela pode ter sido removida da pasta da biblioteca." />
          );
        })()}
    </div>
  );
}

function Editor({ song, initial }: { song: Song; initial: MelodyReference | null }) {
  const toast = useToast();
  const { navigate } = useNav();
  const [ref, setRef] = useState(initial);
  const [offset, setOffset] = useState(initial?.offsetMs ?? 0);
  const [busy, setBusy] = useState(false);
  const [tracks, setTracks] = useState<{ path: string; list: MidiTrackInfo[] } | null>(null);
  const [capturing, setCapturing] = useState(false);
  const dirty = ref != null && offset !== ref.offsetMs;

  const run = async (fn: () => Promise<MelodyReference>, done: string) => {
    setBusy(true);
    try {
      const r = await fn();
      setRef(r);
      setOffset(r.offsetMs);
      toast(done, "success");
    } catch (e) {
      toast(errorMessage(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const doImport = (path: string, track: number | null) =>
    run(() => importReference(song.id, path, track), "Referência importada. Confira o alinhamento e valide.");

  const pickFile = async () => {
    let path: string | null;
    try {
      path = await open({
        multiple: false,
        directory: false,
        title: "Escolha a referência melódica",
        filters: [{ name: "Referência melódica (MIDI ou JSON)", extensions: ["mid", "midi", "json"] }],
      });
    } catch (e) {
      toast(errorMessage(e), "error");
      return;
    }
    if (!path) return;
    if (!/\.midi?$/i.test(path)) return doImport(path, null);
    try {
      const list = (await listMidiTracks(path)).filter((t) => t.noteCount > 0);
      if (list.length === 1) await doImport(path, list[0].index);
      else setTracks({ path, list }); // R1: the user picks the vocal track
    } catch (e) {
      toast(errorMessage(e), "error");
    }
  };

  return (
    <>
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="text-muted">Referência melódica</p>
          <h1 className="font-display text-5xl font-bold">{song.title}</h1>
        </div>
        <div className="flex flex-wrap gap-3">
          <Button onClick={() => navigate({ name: "player", songId: song.id })} disabled={capturing}>
            Cantar
          </Button>
          <Button onClick={() => setCapturing(true)} disabled={busy || capturing}>
            {ref?.source === "manual" ? "Gravar de novo" : "Gravar cantando"}
          </Button>
          <Button variant={ref ? "secondary" : "primary"} onClick={pickFile} disabled={busy || capturing}>
            {ref ? "Importar outra" : "Importar referência"}
          </Button>
        </div>
      </header>

      {capturing ? (
        <SingCapture
          song={song}
          replacing={ref != null}
          onCancel={() => setCapturing(false)}
          onDone={(r) => {
            setRef(r);
            setOffset(r.offsetMs);
            setCapturing(false);
            toast("Notas gravadas. Confira o alinhamento e valide.", "success");
          }}
        />
      ) : !ref ? (
        <Card className="flex max-w-2xl flex-col gap-3">
          <h2 className="font-display text-2xl font-semibold">Sem referência</h2>
          <p className="text-muted">
            Sem a linha melódica não há pontuação competitiva. Importe um arquivo MIDI (escolha a trilha da voz) ou um JSON
            no formato mon-melody v1, ou grave cantando a melodia junto com o vídeo.
          </p>
        </Card>
      ) : (
        <div className="grid gap-6 xl:grid-cols-[2fr_1fr]">
          <AlignPreview song={song} notes={ref.notes} offsetMs={offset} />
          <Card className="flex flex-col gap-5">
            <div className="flex flex-wrap gap-2">
              <Badge tone={referenceLabel[ref.status].tone}>{referenceLabel[ref.status].text}</Badge>
              <Badge>{sourceLabel[ref.source]}</Badge>
              <Badge>Revisão {ref.revision}</Badge>
              <Badge>{ref.notes.length} notas</Badge>
            </div>
            <Slider
              label="Offset das notas"
              min={-OFFSET_LIMIT_MS}
              max={OFFSET_LIMIT_MS}
              step={10}
              value={offset}
              onChange={setOffset}
              format={(v) => `${v > 0 ? "+" : ""}${v} ms`}
            />
            <div role="group" aria-label="Ajuste fino do offset" className="flex flex-wrap gap-2">
              {[-100, -10, 10, 100].map((d) => (
                <Button
                  key={d}
                  variant="ghost"
                  onClick={() => setOffset((o) => Math.max(-OFFSET_LIMIT_MS, Math.min(OFFSET_LIMIT_MS, o + d)))}
                >
                  {d > 0 ? `+${d}` : d} ms
                </Button>
              ))}
            </div>
            <p className="text-sm text-muted">Positivo atrasa as notas; negativo adianta. Salvar volta o status para experimental.</p>
            <div className="flex flex-wrap gap-3">
              <Button
                variant="primary"
                disabled={!dirty || busy}
                onClick={() => run(() => setReferenceOffset(ref.id, offset), "Offset salvo. Confira e valide de novo.")}
              >
                Salvar offset
              </Button>
              {dirty && (
                <Button variant="ghost" onClick={() => setOffset(ref.offsetMs)}>
                  Desfazer
                </Button>
              )}
            </div>
            <hr className="border-line" />
            <p className="text-muted">Quando as notas acompanharem a voz do vídeo, valide para liberar o ranking.</p>
            <div className="flex flex-wrap gap-3">
              <Button
                variant="primary"
                disabled={dirty || busy || ref.status === "validated"}
                onClick={() => run(() => setReferenceStatus(ref.id, "validated"), "Referência validada: partidas valem ranking.")}
              >
                Validar alinhamento
              </Button>
              <Button
                variant="danger"
                disabled={busy || ref.status === "invalid"}
                onClick={() => run(() => setReferenceStatus(ref.id, "invalid"), "Referência marcada como inválida.")}
              >
                Marcar inválida
              </Button>
            </div>
            {dirty && <p className="text-sm text-warning">Salve o offset antes de validar.</p>}
          </Card>
        </div>
      )}

      <Dialog open={tracks != null} onClose={() => setTracks(null)} title="Qual trilha é a voz?">
        <ul className="flex flex-col gap-2">
          {tracks?.list.map((t) => (
            <li key={t.index}>
              <Button
                className="w-full justify-between"
                onClick={() => {
                  const path = tracks.path;
                  setTracks(null);
                  void doImport(path, t.index);
                }}
              >
                <span>{t.name ?? `Trilha ${t.index + 1}`}</span>
                <span className="text-muted">{t.noteCount} notas</span>
              </Button>
            </li>
          ))}
        </ul>
        <div className="mt-6 flex justify-end">
          <Button variant="ghost" onClick={() => setTracks(null)}>
            Cancelar
          </Button>
        </div>
      </Dialog>
    </>
  );
}

type CaptureStep =
  | { step: "intro" }
  | { step: "countdown"; n: number }
  | { step: "recording" }
  | { step: "saving" }
  | { step: "failed"; reason: string };

/** R8: countdown, the video plays from the start while someone sings; Rust saves only the notes as the new active reference. */
function SingCapture({
  song,
  replacing,
  onDone,
  onCancel,
}: {
  song: Song;
  replacing: boolean;
  onDone: (r: MelodyReference) => void;
  onCancel: () => void;
}) {
  const toast = useToast();
  const video = useRef<HTMLVideoElement>(null);
  const open = useRef(false); // a Rust capture is open and must be finished or aborted
  const [state, setState] = useState<CaptureStep>({ step: "intro" });
  const [frame, setFrame] = useState<MicFrame | null>(null);

  useEffect(
    () => () => {
      if (open.current) abortPerformance().catch(warn("abort_performance"));
    },
    [],
  );

  useEffect(() => {
    if (state.step !== "countdown") return;
    const t = setTimeout(() => setState(state.n > 1 ? { step: "countdown", n: state.n - 1 } : { step: "recording" }), 1000);
    return () => clearTimeout(t);
  }, [state]);

  const abort = () => {
    video.current?.pause();
    if (!open.current) return;
    open.current = false;
    abortPerformance().catch(warn("abort_performance"));
  };

  const fail = (reason: string) => {
    abort();
    setState({ step: "failed", reason });
  };

  const recording = state.step === "recording";
  useEffect(() => {
    const v = video.current;
    if (!recording || !v) return;
    v.play().catch((e: unknown) => fail(errorMessage(e)));
    return startVideoSync(v); // media clock, as in the Player
  }, [recording]);

  const sync = (playing: boolean) => {
    if (open.current) syncPerformance(video.current?.currentTime ?? 0, playing, false).catch(warn("sync_performance"));
  };

  const begin = async () => {
    const v = video.current;
    if (!v) return;
    v.currentTime = 0; // before the capture opens: no seek reaches Rust
    try {
      await startPerformance({ songId: song.id, roundId: null, participantId: null, referenceCapture: true }, (t) => setFrame(t.mic));
      open.current = true;
      setState({ step: "countdown", n: COUNTDOWN_S });
    } catch (e) {
      toast(errorMessage(e), "error");
    }
  };

  const stop = async () => {
    const v = video.current;
    if (!open.current || !v) return;
    open.current = false;
    setState({ step: "saving" });
    v.pause();
    await syncPerformance(v.currentTime, false, false).catch(warn("sync_performance"));
    try {
      const outcome = await finishPerformance();
      if (outcome.capturedReference) onDone(outcome.capturedReference);
      else setState({ step: "failed", reason: outcome.reason ?? "Poucas notas detectadas. Nada foi salvo." });
    } catch (e) {
      setState({ step: "failed", reason: errorMessage(e) });
    }
  };

  return (
    <section aria-labelledby="capture-title" className="grid gap-6 xl:grid-cols-[2fr_1fr]">
      <div className="relative flex items-center justify-center overflow-hidden rounded-lg border border-line bg-black">
        <video
          ref={video}
          src={videoSrc(song)}
          preload="auto"
          className="aspect-video w-full"
          aria-label={`Vídeo de ${song.title}`}
          onPlay={() => sync(true)}
          onPause={() => sync(false)}
          onEnded={stop}
          onError={() => fail("Não foi possível tocar o vídeo desta música.")}
        />
        {state.step === "countdown" && (
          <p aria-live="assertive" className="absolute font-display text-[10rem] font-bold text-fg drop-shadow-lg">
            <span key={state.n} className="inline-block animate-enter">
              {state.n}
            </span>
          </p>
        )}
        {recording && (
          <span className="absolute top-4 left-4">
            <Badge tone="danger">Gravando notas</Badge>
          </span>
        )}
      </div>
      <Card className="flex flex-col gap-5">
        <h2 id="capture-title" className="font-display text-2xl font-semibold">
          Gravar cantando
        </h2>
        <p>Cante a melodia junto com o vídeo; o app grava só as notas, nunca a sua voz. Use fones de ouvido.</p>
        {replacing && <p className="text-sm text-muted">A referência atual fica no histórico; a nova nasce experimental.</p>}
        {recording && <MicMeter frame={frame} />}
        <div aria-live="polite">
          {state.step === "saving" && <p className="font-semibold">Salvando as notas…</p>}
          {state.step === "failed" && <p className="text-danger">{state.reason}</p>}
        </div>
        <div className="flex flex-wrap gap-3">
          {(state.step === "intro" || state.step === "failed") && (
            <Button variant="primary" autoFocus onClick={begin}>
              {state.step === "failed" ? "Gravar de novo" : "Começar"}
            </Button>
          )}
          {recording && (
            <Button variant="primary" autoFocus onClick={stop}>
              Parar
            </Button>
          )}
          {state.step !== "saving" && (
            <Button
              variant="ghost"
              onClick={() => {
                abort();
                onCancel();
              }}
            >
              Cancelar
            </Button>
          )}
        </div>
      </Card>
    </section>
  );
}

/** Video with the reference notes drawn over it (R3). Uses video.currentTime (media clock) for the playhead. */
function AlignPreview({ song, notes, offsetMs }: { song: Song; notes: RefNote[]; offsetMs: number }) {
  const video = useRef<HTMLVideoElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const offsetRef = useRef(offsetMs);
  offsetRef.current = offsetMs;

  useEffect(() => {
    const c = canvas.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return; // jsdom / no canvas
    const css = getComputedStyle(document.documentElement);
    const accent = css.getPropertyValue("--color-accent").trim() || "#ff4f8b";
    const focus = css.getPropertyValue("--color-focus").trim() || "#ffe066";
    let raf = 0;
    const draw = () => {
      const w = (c.width = c.clientWidth * devicePixelRatio);
      const h = (c.height = c.clientHeight * devicePixelRatio);
      ctx.clearRect(0, 0, w, h);
      const now = (video.current?.currentTime ?? 0) * 1000;
      for (const b of rollLayout(notes, offsetRef.current, now)) {
        ctx.fillStyle = b.active ? focus : accent;
        ctx.globalAlpha = b.active ? 1 : 0.75;
        ctx.fillRect(b.x * w, b.y * h, Math.max(2, b.w * w), Math.max(2, b.h * h - 2));
      }
      ctx.globalAlpha = 1;
      ctx.fillStyle = "#ffffff";
      const playhead = (ROLL_BEFORE_MS / (ROLL_BEFORE_MS + ROLL_AFTER_MS)) * w;
      ctx.fillRect(playhead - 1, 0, 2, h);
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [notes]);

  return (
    <div className="relative overflow-hidden rounded-lg border border-line bg-black">
      <video ref={video} src={videoSrc(song)} controls className="aspect-video w-full" aria-label={`Vídeo de ${song.title}`} />
      <canvas
        ref={canvas}
        aria-hidden
        className="pointer-events-none absolute inset-x-0 top-0 h-2/5 w-full bg-linear-to-b from-bg/70 to-transparent"
      />
    </div>
  );
}
