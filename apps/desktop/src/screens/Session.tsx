// SPEC-008: a running or finished session. Rust owns rounds/standings; the UI only reads and drives them.
import { Fragment, useState } from "react";
import { ScoreBadge, StandingsTable, formatScore, modeLabel } from "../components/game";
import { Badge, Button, Card, Dialog, ErrorState, LoadingState, Progress, errorMessage, useToast, type Tone } from "../components/ui";
import {
  completeSession,
  getSession,
  listParticipants,
  listSongs,
  setRoundStatus,
  type GameSession,
  type Round,
  type Song,
} from "../lib/ipc";
import { useNav } from "../nav";
import { useAsync } from "../useAsync";

// ---------- pure helpers (tested) ----------
export const currentRound = (s: GameSession): Round | null =>
  s.status !== "active" ? null : ([...s.rounds].sort((a, b) => a.position - b.position).find((r) => r.status === "pending") ?? null);

export type DuelResult = { kind: "pending" } | { kind: "tie"; score: number } | { kind: "win"; winnerId: number; scores: Map<number, number> };
export function duelResult(s: GameSession): DuelResult {
  const done = s.rounds.filter((r) => r.status === "completed" && r.score != null && r.participantId != null);
  if (done.length < 2) return { kind: "pending" };
  const [a, b] = done;
  if (a.score === b.score) return { kind: "tie", score: a.score! };
  const win = a.score! > b.score! ? a : b;
  return { kind: "win", winnerId: win.participantId!, scores: new Map(done.map((r) => [r.participantId!, r.score!])) };
}

export interface ChallengeStatus {
  played: number; // completed + skipped
  total: number;
  average: number | null; // over completed rounds
  target: number;
  state: "running" | "success" | "failure";
}
export function challengeStatus(s: GameSession): ChallengeStatus {
  const completed = s.rounds.filter((r) => r.status === "completed" && r.score != null);
  const played = s.rounds.filter((r) => r.status !== "pending").length;
  const average = completed.length ? completed.reduce((t, r) => t + r.score!, 0) / completed.length : null;
  const target = s.config.targetScore ?? 0;
  const over = played === s.rounds.length || s.status !== "active";
  const state = !over ? "running" : average != null && average >= target ? "success" : "failure";
  return { played, total: s.rounds.length, average, target, state };
}

const statusText: Record<GameSession["status"], { text: string; tone: Tone }> = {
  active: { text: "Em andamento", tone: "accent" },
  completed: { text: "Encerrada", tone: "success" },
  abandoned: { text: "Abandonada", tone: "neutral" },
};
const roundText: Record<Round["status"], { text: string; tone: Tone }> = {
  pending: { text: "Pendente", tone: "neutral" },
  completed: { text: "Concluída", tone: "success" },
  skipped: { text: "Pulada", tone: "warning" },
};
const dateFmt = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short", timeStyle: "short" });

// ---------- screen ----------
export function Session({ id }: { id: number }) {
  const [load, reload] = useAsync(() => Promise.all([getSession(id), listParticipants(), listSongs()]), [id]);
  if (load.status === "loading") return <LoadingState label="Carregando partida…" />;
  if (load.status === "error") return <ErrorState title="Não foi possível abrir a partida" error={load.error} onRetry={reload} />;
  const [session, people, songs] = load.data;
  return (
    <View
      initial={session}
      names={new Map(people.map((p) => [p.id, p.name]))}
      songs={new Map(songs.map((s) => [s.id, s]))}
    />
  );
}

function View({ initial, names, songs }: { initial: GameSession; names: Map<number, string>; songs: Map<string, Song> }) {
  const { navigate } = useNav();
  const toast = useToast();
  const [s, setS] = useState(initial);
  const [confirmEnd, setConfirmEnd] = useState(false);
  const [busy, setBusy] = useState(false);
  const now = currentRound(s);
  const pending = s.rounds.filter((r) => r.status === "pending").length;
  const who = (id: number | null) => (id == null ? "Cantor livre" : (names.get(id) ?? `#${id}`));
  const songTitle = (id: string) => songs.get(id)?.title ?? "Música removida";

  const act = async (fn: () => Promise<GameSession>) => {
    setBusy(true);
    try {
      setS(await fn());
    } catch (e) {
      toast(errorMessage(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const sing = (r: Round) =>
    navigate({ name: "player", songId: r.songId, round: { sessionId: s.id, roundId: r.id, participantId: r.participantId } });

  return (
    <div className="flex flex-col gap-8">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="text-muted">
            {modeLabel[s.mode]} · iniciada em {dateFmt.format(s.startedMs)}
          </p>
          <h1 className="font-display text-5xl font-bold">{s.name ?? modeLabel[s.mode]}</h1>
        </div>
        <div className="flex items-center gap-3">
          <Badge tone={statusText[s.status].tone}>{statusText[s.status].text}</Badge>
          {s.status === "active" && (
            <Button variant="ghost" disabled={busy} onClick={() => (pending ? setConfirmEnd(true) : act(() => completeSession(s.id)))}>
              Encerrar
            </Button>
          )}
        </div>
      </header>

      {now && (
        <Card className="flex flex-wrap items-center justify-between gap-6 border-accent/40 shadow-glow">
          <div>
            <p className="text-muted">
              Rodada {now.position} de {s.rounds.length}
            </p>
            <h2 className="font-display text-3xl font-bold">
              {who(now.participantId)} <span className="text-muted">canta</span> {songTitle(now.songId)}
            </h2>
          </div>
          <div className="flex gap-3">
            <Button variant="ghost" disabled={busy} onClick={() => act(() => setRoundStatus(now.id, "skipped"))}>
              Pular
            </Button>
            <Button variant="primary" size="lg" autoFocus disabled={busy || !songs.get(now.songId)?.available} onClick={() => sing(now)}>
              Cantar
            </Button>
          </div>
        </Card>
      )}

      {s.mode === "party" && <PartyBoard s={s} names={names} />}
      {s.mode === "duel" && <DuelBoard s={s} who={who} />}
      {s.mode === "challenge" && <ChallengeBoard s={s} />}

      <section aria-labelledby="rounds-title" className="flex flex-col gap-3">
        <h2 id="rounds-title" className="font-display text-2xl font-semibold">
          Rodadas
        </h2>
        <ol className="flex flex-col gap-2">
          {[...s.rounds]
            .sort((a, b) => a.position - b.position)
            .map((r) => (
              <li
                key={r.id}
                aria-current={r.id === now?.id ? "step" : undefined}
                className="flex flex-wrap items-center gap-4 rounded-md bg-surface-2 px-4 py-2 aria-[current=step]:outline-2 aria-[current=step]:outline-accent"
              >
                <span className="w-8 font-display font-bold tabular-nums">{r.position}</span>
                <span className="flex-1">
                  {who(r.participantId)} · <span className="text-muted">{songTitle(r.songId)}</span>
                </span>
                {r.score != null && <span className="font-semibold tabular-nums">{formatScore(r.score)}</span>}
                <Badge tone={roundText[r.status].tone}>{roundText[r.status].text}</Badge>
                {s.status === "active" && r.status === "skipped" && (
                  <Button variant="ghost" disabled={busy} onClick={() => act(() => setRoundStatus(r.id, "pending"))}>
                    Voltar para a fila
                  </Button>
                )}
              </li>
            ))}
        </ol>
        <p className="text-sm text-muted">Uma rodada só conta como concluída quando a apresentação vale ranking (sem pular trechos).</p>
      </section>

      <Dialog open={confirmEnd} onClose={() => setConfirmEnd(false)} title="Encerrar a partida?">
        <p className="mb-6 text-muted">
          Ainda {pending === 1 ? "falta 1 rodada" : `faltam ${pending} rodadas`}. O resultado fica com o que foi cantado até agora.
        </p>
        <div className="flex justify-end gap-3">
          <Button variant="ghost" onClick={() => setConfirmEnd(false)}>
            Continuar jogando
          </Button>
          <Button
            variant="primary"
            onClick={() => {
              setConfirmEnd(false);
              void act(() => completeSession(s.id));
            }}
          >
            Encerrar agora
          </Button>
        </div>
      </Dialog>
    </div>
  );
}

function PartyBoard({ s, names }: { s: GameSession; names: Map<number, string> }) {
  const winner = s.status === "completed" ? s.standings.find((x) => x.eligible && x.rank === 1) : undefined;
  return (
    <section aria-labelledby="standings-title" className="flex flex-col gap-3">
      {winner && (
        <Card className="flex flex-col items-center gap-2 text-center">
          <p className="text-muted">Vencedor da festa</p>
          <p className="font-display text-5xl font-bold">{names.get(winner.participantId)}</p>
          <ScoreBadge score={winner.average} />
          <p className="text-sm text-muted">média de {winner.rounds} rodadas</p>
        </Card>
      )}
      <h2 id="standings-title" className="font-display text-2xl font-semibold">
        {s.status === "active" ? "Ranking parcial" : "Classificação final"}
      </h2>
      <p className="text-sm text-muted">
        Classificação pela média. Para entrar, são necessárias pelo menos {s.config.minRounds ?? 1}{" "}
        {(s.config.minRounds ?? 1) === 1 ? "rodada" : "rodadas"}.
      </p>
      <StandingsTable caption="Ranking da festa" standings={s.standings} names={names} minRounds={s.config.minRounds} />
    </section>
  );
}

function DuelBoard({ s, who }: { s: GameSession; who: (id: number | null) => string }) {
  const result = duelResult(s);
  const sides = [...s.rounds].sort((a, b) => a.position - b.position);
  return (
    <section aria-label="Confronto" className="flex flex-col gap-4">
      <div className="grid grid-cols-[1fr_auto_1fr] items-center gap-6">
        {sides.map((r, i) => (
          <Fragment key={r.id}>
            {i === 1 && (
              <span aria-hidden className="font-display text-4xl font-bold text-muted">
                VS
              </span>
            )}
            <Card
              className={`flex flex-col items-center gap-3 text-center ${result.kind === "win" && result.winnerId === r.participantId ? "border-accent shadow-glow" : ""}`}
            >
              <p className="font-display text-3xl font-bold">{who(r.participantId)}</p>
              {r.score != null ? <ScoreBadge score={r.score} /> : <Badge>{roundText[r.status].text}</Badge>}
            </Card>
          </Fragment>
        ))}
      </div>
      <p aria-live="polite" className="text-center font-display text-2xl font-semibold">
        {result.kind === "win" && `${who(result.winnerId)} venceu o duelo!`}
        {result.kind === "tie" && `Empate em ${formatScore(result.score)}!`}
        {result.kind === "pending" && "O duelo termina quando os dois cantarem."}
      </p>
    </section>
  );
}

function ChallengeBoard({ s }: { s: GameSession }) {
  const c = challengeStatus(s);
  return (
    <Card className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="text-muted">Meta de média</p>
          <p className="font-display text-4xl font-bold tabular-nums">{formatScore(c.target)}</p>
        </div>
        <div className="text-right">
          <p className="text-muted">Sua média</p>
          <p className="font-display text-4xl font-bold tabular-nums">{c.average == null ? "–" : formatScore(c.average)}</p>
        </div>
      </div>
      <Progress label="Progresso do desafio" value={c.played} max={Math.max(1, c.total)} />
      <p aria-live="polite" className={c.state === "success" ? "text-success" : c.state === "failure" ? "text-danger" : "text-muted"}>
        {c.state === "running" && `${c.played} de ${c.total} músicas cantadas.`}
        {c.state === "success" && "Desafio cumprido!"}
        {c.state === "failure" && "Desafio não cumprido desta vez."}
      </p>
    </Card>
  );
}
