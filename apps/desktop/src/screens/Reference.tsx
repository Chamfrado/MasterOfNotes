// SPEC-005 T-005-04: import (.mid/.midi/.json), pick the MIDI track, align the offset over the video, validate.
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { Badge, Button, Card, Dialog, ErrorState, LoadingState, Slider, errorMessage, useToast } from "../components/ui";
import {
  getReference,
  importReference,
  listMidiTracks,
  listSongs,
  setReferenceOffset,
  setReferenceStatus,
  videoSrc,
  type MelodyReference,
  type MidiTrackInfo,
  type RefNote,
  type Song,
} from "../lib/ipc";
import { useNav } from "../nav";
import { useAsync } from "../useAsync";
import { referenceLabel } from "./Library";

const OFFSET_LIMIT_MS = 10_000; // R4
const ROLL_BEFORE_MS = 2_000;
const ROLL_AFTER_MS = 4_000;

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
        <div className="flex gap-3">
          <Button onClick={() => navigate({ name: "player", songId: song.id })}>Cantar</Button>
          <Button variant={ref ? "secondary" : "primary"} onClick={pickFile} disabled={busy}>
            {ref ? "Importar outra" : "Importar referência"}
          </Button>
        </div>
      </header>

      {!ref ? (
        <Card className="flex max-w-2xl flex-col gap-3">
          <h2 className="font-display text-2xl font-semibold">Sem referência</h2>
          <p className="text-muted">
            Sem a linha melódica não há pontuação competitiva. Importe um arquivo MIDI (escolha a trilha da voz) ou um JSON
            no formato mon-melody v1.
          </p>
        </Card>
      ) : (
        <div className="grid gap-6 xl:grid-cols-[2fr_1fr]">
          <AlignPreview song={song} notes={ref.notes} offsetMs={offset} />
          <Card className="flex flex-col gap-5">
            <div className="flex flex-wrap gap-2">
              <Badge tone={referenceLabel[ref.status].tone}>{referenceLabel[ref.status].text}</Badge>
              <Badge>{ref.source.toUpperCase()}</Badge>
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
