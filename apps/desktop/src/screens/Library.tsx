// SPEC-002 T-002-04: library screen. Data only from ipc.ts; search is local over listSongs (R10).
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState, type FormEvent } from "react";
import { Badge, Button, Card, Dialog, ErrorState, Input, LoadingState, Progress, errorMessage, useToast, type Tone } from "../components/ui";
import {
  getSettings,
  listSongs,
  onLibraryChanged,
  rescanLibrary,
  setLibraryFolder,
  updateSong,
  type ReferenceStatus,
  type ScanSummary,
  type Song,
} from "../lib/ipc";
import { useNav } from "../nav";
import { useAsync } from "../useAsync";

/** Case- and accent-insensitive key: "Canção" → "cancao". */
export const searchKey = (s: string) => s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
export const songMatches = (song: Song, query: string) => {
  const q = searchKey(query.trim());
  return !q || [song.title, song.artist ?? "", song.fileName].some((f) => searchKey(f).includes(q));
};

export const referenceLabel: Record<ReferenceStatus, { text: string; tone: Tone }> = {
  none: { text: "Sem referência", tone: "neutral" },
  processing: { text: "Processando referência", tone: "info" },
  experimental: { text: "Referência experimental", tone: "warning" },
  validated: { text: "Referência validada", tone: "success" },
  invalid: { text: "Referência inválida", tone: "danger" },
};

export const formatDuration = (ms: number | null) => {
  if (ms == null) return null;
  const s = Math.round(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

const summaryText = (s: ScanSummary) =>
  `${s.found} ${s.found === 1 ? "música encontrada" : "músicas encontradas"} (${s.added} novas, ${s.removed} removidas)`;

export function Library() {
  const toast = useToast();
  const [songs, reloadSongs] = useAsync(listSongs, []);
  const [settings, reloadSettings] = useAsync(getSettings, []);
  const [query, setQuery] = useState("");
  const [view, setView] = useState<"grid" | "list">("grid");
  const [scanning, setScanning] = useState(false);
  const [editing, setEditing] = useState<Song | null>(null);

  // R7: watcher events refresh the list without restarting anything.
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    onLibraryChanged(() => reloadSongs()).then(
      (u) => (alive ? (unlisten = u) : u()),
      (e: unknown) => console.warn("library://changed unavailable:", errorMessage(e)),
    );
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [reloadSongs]);

  const scan = async (run: () => Promise<ScanSummary>) => {
    setScanning(true);
    try {
      toast(summaryText(await run()), "success");
      reloadSongs();
      reloadSettings();
    } catch (e) {
      toast(errorMessage(e), "error");
    } finally {
      setScanning(false);
    }
  };

  const chooseFolder = async () => {
    let dir: string | null;
    try {
      dir = await open({ directory: true, multiple: false, title: "Escolha a pasta dos seus vídeos de karaokê" });
    } catch (e) {
      toast(errorMessage(e), "error");
      return;
    }
    if (!dir) return;
    const recursive = settings.status === "ready" ? settings.data.recursiveScan : true;
    await scan(() => setLibraryFolder(dir, recursive));
  };

  const visible = useMemo(
    () => (songs.status === "ready" ? songs.data.filter((s) => s.available && songMatches(s, query)) : []),
    [songs, query],
  );
  const folder = settings.status === "ready" ? settings.data.libraryFolder : null;
  const total = songs.status === "ready" ? songs.data.filter((s) => s.available).length : 0;

  return (
    <div className="flex flex-col gap-8">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-5xl font-bold">Biblioteca</h1>
          <p className="mt-2 text-muted">{folder ? <>Pasta: <span className="text-fg">{folder}</span></> : "Nenhuma pasta escolhida"}</p>
        </div>
        <div className="flex gap-3">
          {folder && (
            <Button onClick={() => scan(rescanLibrary)} disabled={scanning}>
              Atualizar
            </Button>
          )}
          <Button variant="primary" onClick={chooseFolder} disabled={scanning}>
            {folder ? "Trocar pasta" : "Escolher pasta"}
          </Button>
        </div>
      </header>

      {scanning && (
        <div className="flex flex-col gap-2" aria-live="polite">
          <p className="text-muted">Procurando vídeos…</p>
          <div className="max-w-sm">
            <Progress label="Varredura da biblioteca" />
          </div>
        </div>
      )}

      {songs.status === "loading" && <LoadingState label="Carregando biblioteca…" />}
      {songs.status === "error" && <ErrorState title="Não foi possível carregar a biblioteca" error={songs.error} onRetry={reloadSongs} />}

      {songs.status === "ready" && total === 0 && !scanning && (
        <Card className="flex max-w-2xl flex-col items-start gap-4 self-center text-center">
          <h2 className="font-display text-2xl font-semibold">
            {folder ? "Nenhum vídeo encontrado" : "Comece escolhendo sua pasta de músicas"}
          </h2>
          <p className="text-muted">
            {folder
              ? "A pasta não tem vídeos compatíveis. Formatos aceitos: MP4, MKV, WEBM, MOV e AVI."
              : "Os vídeos ficam onde estão: o Master Of Notes só guarda o caminho e organiza tudo para você."}
          </p>
          <Button variant="primary" size="lg" onClick={chooseFolder}>
            Escolher pasta
          </Button>
        </Card>
      )}

      {songs.status === "ready" && total > 0 && (
        <>
          <div className="flex flex-wrap items-end gap-4">
            <div className="min-w-64 flex-1">
              <Input label="Buscar músicas" type="search" placeholder="Título, artista ou arquivo" value={query} onChange={(e) => setQuery(e.currentTarget.value)} />
            </div>
            <div role="group" aria-label="Modo de exibição" className="flex gap-1">
              <Button variant={view === "grid" ? "secondary" : "ghost"} aria-pressed={view === "grid"} onClick={() => setView("grid")}>
                Grade
              </Button>
              <Button variant={view === "list" ? "secondary" : "ghost"} aria-pressed={view === "list"} onClick={() => setView("list")}>
                Lista
              </Button>
            </div>
          </div>
          <p aria-live="polite" className="text-sm text-muted">
            {visible.length === total ? `${total} músicas` : `${visible.length} de ${total} músicas`}
          </p>
          {visible.length === 0 ? (
            <p className="py-12 text-center text-muted">Nada encontrado para “{query}”.</p>
          ) : (
            <ul className={view === "grid" ? "grid grid-cols-[repeat(auto-fill,minmax(18rem,1fr))] gap-4" : "flex flex-col gap-2"}>
              {visible.map((s) => (
                <li key={s.id}>
                  <SongItem song={s} compact={view === "list"} onEdit={() => setEditing(s)} />
                </li>
              ))}
            </ul>
          )}
        </>
      )}

      <EditSongDialog
        song={editing}
        onClose={() => setEditing(null)}
        onSaved={() => {
          setEditing(null);
          reloadSongs();
        }}
      />
    </div>
  );
}

function SongItem({ song, compact, onEdit }: { song: Song; compact: boolean; onEdit: () => void }) {
  const { navigate } = useNav();
  const ref = referenceLabel[song.referenceStatus];
  const duration = formatDuration(song.durationMs);
  const unplayable = song.playable === "no";
  return (
    <div
      className={`group flex gap-4 rounded-lg border border-line bg-surface shadow-card transition-transform duration-(--duration-base) ease-out focus-within:-translate-y-0.5 hover:-translate-y-0.5 ${compact ? "items-center p-3" : "flex-col p-5"}`}
    >
      <button
        type="button"
        onClick={() => navigate({ name: "song", songId: song.id })}
        className="flex min-h-12 flex-1 cursor-pointer flex-col items-start gap-1 rounded-md text-left"
      >
        <span className="line-clamp-2 font-display text-xl font-semibold">
          <span className="sr-only">Detalhes de</span> {song.title}
        </span>{" "}
        <span className="text-muted">{song.artist ?? song.fileName}</span>
      </button>
      <div className={`flex flex-wrap items-center gap-2 ${compact ? "" : "justify-between"}`}>
        <div className="flex flex-wrap gap-2">
          <Badge tone={ref.tone}>{ref.text}</Badge>
          {unplayable && <Badge tone="danger">Formato incompatível</Badge>}
          <Badge>{song.extension.toUpperCase()}</Badge>
          {duration && <Badge>{duration}</Badge>}
        </div>
        <div className="flex flex-wrap gap-1">
          <Button variant="primary" disabled={unplayable} onClick={() => navigate({ name: "player", songId: song.id })}>
            Cantar{" "}
            <span className="sr-only">
              {song.title} {song.artist ?? song.fileName}
            </span>
          </Button>
          <Button variant="ghost" onClick={() => navigate({ name: "reference", songId: song.id })} aria-label={`Referência de ${song.title}`}>
            Referência
          </Button>
          <Button variant="ghost" onClick={onEdit} aria-label={`Editar ${song.title}`}>
            Editar
          </Button>
        </div>
      </div>
    </div>
  );
}

/** Title/artist form of a song (library dialog and song page): reset when `song` changes. */
export function useSongEdit(song: Song | null, onSaved: (updated: Song) => void) {
  const toast = useToast();
  const [title, setTitle] = useState(song?.title ?? "");
  const [artist, setArtist] = useState(song?.artist ?? "");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    setTitle(song?.title ?? "");
    setArtist(song?.artist ?? "");
  }, [song]);
  const titleError = title.trim() ? undefined : "Informe um título.";
  const changed = song != null && (title.trim() !== song.title || (artist.trim() || null) !== song.artist);
  const save = async (e: FormEvent) => {
    e.preventDefault();
    if (!song || titleError) return;
    setSaving(true);
    try {
      const updated = await updateSong(song.id, { title: title.trim(), artist: artist.trim() || null });
      toast("Música atualizada.", "success");
      onSaved(updated);
    } catch (err) {
      toast(errorMessage(err), "error");
    } finally {
      setSaving(false);
    }
  };
  return { title, setTitle, artist, setArtist, titleError, changed, saving, save };
}

function EditSongDialog({ song, onClose, onSaved }: { song: Song | null; onClose: () => void; onSaved: () => void }) {
  const { title, setTitle, artist, setArtist, titleError, saving, save } = useSongEdit(song, onSaved);
  return (
    <Dialog open={song != null} onClose={onClose} title="Editar música">
      <form onSubmit={save} className="flex flex-col gap-5">
        <Input label="Título" value={title} onChange={(e) => setTitle(e.currentTarget.value)} error={titleError} required />
        <Input label="Artista" hint="Opcional" value={artist} onChange={(e) => setArtist(e.currentTarget.value)} />
        <div className="flex justify-end gap-3">
          <Button variant="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button type="submit" variant="primary" disabled={saving || !!titleError}>
            Salvar
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
