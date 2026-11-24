// SPEC-010 T-010-06: song details. Edit title/artist, file info, playability, reference shortcut, records.
import { useState } from "react";
import { RankingTable, ScoreBadge, difficultyLabel, formatMB, formatScore } from "../components/game";
import { Badge, Button, Card, ErrorState, Input, LoadingState, Select, errorMessage } from "../components/ui";
import {
  getLeaderboard,
  getSettings,
  listParticipants,
  listPerformances,
  listSongs,
  type Difficulty,
  type Participant,
  type PerformanceRow,
  type Song,
} from "../lib/ipc";
import { useNav } from "../nav";
import { useAsync } from "../useAsync";
import { formatDuration, referenceLabel, useSongEdit } from "./Library";
import { CODEC_HINT } from "./Player";

const TOP = 5;
const playableText: Record<Song["playable"], string> = {
  yes: "Compatível: o vídeo abre neste computador.",
  unknown: "Ainda não verificado: confirmamos na primeira vez que tocar.",
  no: CODEC_HINT,
};
const referenceHelp: Record<Song["referenceStatus"], string> = {
  none: "Sem a linha melódica não há pontuação. Importe um MIDI ou JSON.",
  processing: "A referência está sendo processada.",
  experimental: "Confira o alinhamento sobre o vídeo e valide para liberar o ranking.",
  validated: "Pontuação competitiva liberada.",
  invalid: "A referência foi marcada como inválida. Importe outra.",
};

export function SongDetails({ songId }: { songId: string }) {
  const [load, reload] = useAsync(() => Promise.all([listSongs(), getSettings()]), [songId]);
  if (load.status === "loading") return <LoadingState label="Carregando música…" />;
  if (load.status === "error") return <ErrorState title="Não foi possível abrir a música" error={load.error} onRetry={reload} />;
  const song = load.data[0].find((s) => s.id === songId && s.available);
  if (!song) return <ErrorState title="Música não encontrada" error="Ela pode ter sido removida da pasta da biblioteca." />;
  return <Details initial={song} difficulty={load.data[1].difficulty} />;
}

function Details({ initial, difficulty }: { initial: Song; difficulty: Difficulty }) {
  const { navigate } = useNav();
  const [song, setSong] = useState(initial);
  const { title, setTitle, artist, setArtist, titleError, changed, saving, save } = useSongEdit(song, setSong);
  const ref = referenceLabel[song.referenceStatus];
  const duration = formatDuration(song.durationMs);

  return (
    <div className="flex flex-col gap-8">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-5xl font-bold">{song.title}</h1>
          <p className="mt-1 text-xl text-muted">{song.artist ?? "Artista não informado"}</p>
        </div>
        <div className="flex gap-3">
          <Button size="lg" onClick={() => navigate({ name: "ranking", songId: song.id })}>
            Ranking completo
          </Button>
          <Button variant="primary" size="lg" autoFocus disabled={song.playable === "no"} onClick={() => navigate({ name: "player", songId: song.id })}>
            Cantar
          </Button>
        </div>
      </header>

      <div className="grid gap-6 xl:grid-cols-2">
        <Card className="flex flex-col gap-6">
          <form onSubmit={save} className="flex flex-col gap-4">
            <h2 className="font-display text-2xl font-semibold">Informações</h2>
            <Input label="Título" value={title} onChange={(e) => setTitle(e.currentTarget.value)} error={titleError} required />
            <Input label="Artista" hint="Opcional" value={artist} onChange={(e) => setArtist(e.currentTarget.value)} />
            <Button type="submit" className="self-start" disabled={saving || !!titleError || !changed}>
              Salvar alterações
            </Button>
          </form>
          <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2">
            <dt className="text-muted">Arquivo</dt>
            <dd className="break-all">{song.fileName}</dd>
            <dt className="text-muted">Formato</dt>
            <dd>{song.extension.toUpperCase()}</dd>
            <dt className="text-muted">Duração</dt>
            <dd>{duration ?? "Desconhecida até tocar"}</dd>
            <dt className="text-muted">Tamanho</dt>
            <dd>{formatMB(song.sizeBytes)}</dd>
          </dl>
          <section aria-labelledby="play-title" className="flex flex-col gap-2">
            <h3 id="play-title" className="font-semibold">
              Reprodução
            </h3>
            <p className={song.playable === "no" ? "text-danger" : "text-muted"}>{playableText[song.playable]}</p>
          </section>
        </Card>

        <div className="flex flex-col gap-6">
          <Card className="flex flex-col items-start gap-3">
            <div className="flex w-full items-center justify-between gap-3">
              <h2 className="font-display text-2xl font-semibold">Referência melódica</h2>
              <Badge tone={ref.tone}>{ref.text}</Badge>
            </div>
            <p className="text-muted">{referenceHelp[song.referenceStatus]}</p>
            <Button onClick={() => navigate({ name: "reference", songId: song.id })}>
              {song.referenceStatus === "none" ? "Importar referência" : "Abrir referência"}
            </Button>
          </Card>
          <Records song={song} difficulty={difficulty} />
        </div>
      </div>
      <Performances song={song} />
    </div>
  );
}

/** Top 5 and records for the current difficulty. */
function Records({ song, difficulty }: { song: Song; difficulty: Difficulty }) {
  const [load, reload] = useAsync(() => Promise.all([getLeaderboard(song.id, difficulty), listParticipants()]), [song.id, difficulty]);
  const [who, setWho] = useState("");
  return (
    <Card className="flex flex-col gap-4">
      <h2 className="font-display text-2xl font-semibold">
        Recordes <span className="text-base font-normal text-muted">({difficultyLabel[difficulty]})</span>
      </h2>
      {load.status === "loading" && <LoadingState label="Carregando recordes…" />}
      {load.status === "error" && <ErrorState title="Não foi possível carregar os recordes" error={load.error} onRetry={reload} />}
      {load.status === "ready" &&
        (() => {
          const [rows, people] = load.data;
          return (
            <>
              {rows[0] && (
                <div className="flex items-center gap-4">
                  <ScoreBadge score={rows[0].score} />
                  <p>
                    Recorde da música: <strong>{rows[0].participantName}</strong>
                  </p>
                </div>
              )}
              <RankingTable rows={rows.slice(0, TOP)} caption={`Top ${TOP} de ${song.title}`} />
              {people.length > 0 && (
                <div className="flex flex-col gap-2">
                  <Select
                    label="Recorde pessoal de"
                    value={who}
                    onChange={(e) => setWho(e.currentTarget.value)}
                    options={[{ value: "", label: "Escolha um cantor" }, ...people.map((p) => ({ value: String(p.id), label: p.name }))]}
                  />
                  {who && <PersonalBest song={song} difficulty={difficulty} singer={people.find((p) => String(p.id) === who)!} />}
                </div>
              )}
            </>
          );
        })()}
    </Card>
  );
}

/** Best ranked score: competitive, same CompatKey as today, at the screen's difficulty. null = none yet. */
export function personalBest(rows: PerformanceRow[], difficulty: Difficulty): number | null {
  let best: number | null = null;
  for (const r of rows) if (r.competitive && r.isCurrentKey && r.difficulty === difficulty && (best == null || r.score > best)) best = r.score;
  return best;
}

const PERSONAL_LIMIT = 500; // listPerformances max

function PersonalBest({ song, difficulty, singer }: { song: Song; difficulty: Difficulty; singer: Participant }) {
  const [load] = useAsync(
    () => listPerformances({ songId: song.id, participantId: singer.id, limit: PERSONAL_LIMIT }),
    [song.id, singer.id],
  );
  if (load.status === "loading") return <p className="text-muted">Procurando…</p>;
  if (load.status === "error") return <p className="text-sm text-danger">{errorMessage(load.error)}</p>;
  const best = personalBest(load.data, difficulty);
  return (
    <p aria-live="polite">
      {best == null
        ? `${singer.name} ainda não tem pontuação valendo ranking nesta música (${difficultyLabel[difficulty]}).`
        : `${singer.name}: ${formatScore(best)} pontos.`}
    </p>
  );
}

const RECENT = 10;
const dateTime = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short", timeStyle: "short" });

/** SPEC-007 R6: last performances of this song; older CompatKeys flagged, practice runs flagged. */
function Performances({ song }: { song: Song }) {
  const [load] = useAsync(() => listPerformances({ songId: song.id, limit: RECENT }), [song.id]);
  return (
    <section aria-labelledby="perf-title" className="flex flex-col gap-3">
      <h2 id="perf-title" className="font-display text-2xl font-semibold">
        Suas apresentações
      </h2>
      {load.status === "loading" && <LoadingState label="Carregando apresentações…" />}
      {/* Discreet: history is secondary; never replace it with made-up rows. */}
      {load.status === "error" && (
        <p className="text-sm text-muted">Histórico indisponível no momento ({errorMessage(load.error)}).</p>
      )}
      {load.status === "ready" &&
        (load.data.length === 0 ? (
          <p className="text-muted">Ninguém cantou esta música ainda.</p>
        ) : (
          <ul className="flex flex-col gap-2">
            {load.data.map((p) => (
              <li key={p.performanceId} className="flex flex-wrap items-center gap-3 rounded-md bg-surface-2 px-4 py-3">
                <span className="w-16 font-display text-xl font-bold tabular-nums">{formatScore(p.score)}</span>
                <span className="flex-1">
                  {p.participantName ?? "Anônimo"}{" "}
                  <span className="text-muted">
                    · {difficultyLabel[p.difficulty]} · {dateTime.format(p.createdMs)}
                  </span>
                </span>
                {!p.competitive && <Badge tone="warning">Treino</Badge>}
                {!p.isCurrentKey && <Badge>Versão anterior</Badge>}
              </li>
            ))}
          </ul>
        ))}
    </section>
  );
}

