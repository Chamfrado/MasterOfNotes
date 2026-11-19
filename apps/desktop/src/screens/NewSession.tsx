// SPEC-008: create a Solo / Party / Duel / Challenge session. Rules mirror create_session (Rust validates too).
import { useMemo, useState, type FormEvent } from "react";
import { modeLabel } from "../components/game";
import { Badge, Button, Card, ErrorState, Input, LoadingState, errorMessage, useToast } from "../components/ui";
import { createSession, listParticipants, listSongs, type GameMode, type Participant, type Song } from "../lib/ipc";
import { useNav } from "../nav";
import { useAsync } from "../useAsync";
import { songMatches } from "./Library";
import { AddParticipant } from "./Participants";

export interface SessionForm {
  name: string;
  participantIds: number[]; // selection order = manual order
  songIds: string[];
  order: "manual" | "shuffle";
  minRounds: string; // "" = default (fewest rounds assigned to a participant)
  targetScore: string;
}

const rules: Record<GameMode, { singers: [number, number]; songs: [number, number]; validatedOnly: boolean }> = {
  solo: { singers: [0, 1], songs: [1, 1], validatedOnly: false },
  party: { singers: [2, Infinity], songs: [1, Infinity], validatedOnly: true },
  duel: { singers: [2, 2], songs: [1, 1], validatedOnly: true },
  challenge: { singers: [0, 1], songs: [1, Infinity], validatedOnly: true },
};
export const songAllowed = (mode: GameMode, s: Song) =>
  s.available && s.playable !== "no" && (!rules[mode].validatedOnly || s.referenceStatus === "validated");

const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
function rangeText(range: [number, number], one: string, many: string) {
  const [lo, hi] = range;
  if (lo === hi) return `exatamente ${count(lo, one, many)}`;
  if (hi === Infinity) return `pelo menos ${count(lo, one, many)}`;
  return lo === 0 ? `no máximo ${count(hi, one, many)}` : `de ${lo} a ${count(hi, one, many)}`;
}

/** pt-BR problems that block creation; empty = valid. */
export function validateSession(mode: GameMode, f: SessionForm): string[] {
  const r = rules[mode];
  const errors: string[] = [];
  if (mode === "party" && !f.name.trim()) errors.push("Dê um nome à festa.");
  const p = f.participantIds.length;
  if (p < r.singers[0] || p > r.singers[1]) errors.push(`Escolha ${rangeText(r.singers, "cantor", "cantores")}.`);
  const s = f.songIds.length;
  if (s < r.songs[0] || s > r.songs[1]) errors.push(`Escolha ${rangeText(r.songs, "música", "músicas")}.`);
  if (mode === "party" && f.minRounds !== "") {
    const m = Number(f.minRounds);
    if (!Number.isInteger(m) || m < 1) errors.push("Mínimo de rodadas deve ser um número inteiro a partir de 1.");
  }
  if (mode === "challenge") {
    const t = Number(f.targetScore);
    if (f.targetScore === "" || !Number.isFinite(t) || t < 0 || t > 100) errors.push("Meta de pontuação deve ficar entre 0 e 100.");
  }
  return errors;
}

const toggle = <T,>(xs: T[], x: T, max: number) =>
  xs.includes(x) ? xs.filter((y) => y !== x) : max === 1 ? [x] : xs.length < max ? [...xs, x] : xs;

export function NewSession({ mode }: { mode: GameMode }) {
  const [load, reload] = useAsync(() => Promise.all([listParticipants(), listSongs()]), []);
  return (
    <div className="flex flex-col gap-8">
      <h1 className="font-display text-5xl font-bold">Nova partida: {modeLabel[mode]}</h1>
      {load.status === "loading" && <LoadingState />}
      {load.status === "error" && <ErrorState title="Não foi possível preparar a partida" error={load.error} onRetry={reload} />}
      {load.status === "ready" && <Form mode={mode} initialPeople={load.data[0]} songs={load.data[1]} />}
    </div>
  );
}

function Form({ mode, initialPeople, songs }: { mode: GameMode; initialPeople: Participant[]; songs: Song[] }) {
  const { replace } = useNav();
  const toast = useToast();
  const r = rules[mode];
  const [people, setPeople] = useState(initialPeople);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [f, setF] = useState<SessionForm>({ name: "", participantIds: [], songIds: [], order: "shuffle", minRounds: "", targetScore: "80" });
  const errors = validateSession(mode, f);
  const blocked = songs.filter((s) => s.available && !songAllowed(mode, s)).length;
  const visibleSongs = useMemo(() => songs.filter((s) => songAllowed(mode, s) && songMatches(s, query)), [songs, mode, query]);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (errors.length) return;
    setBusy(true);
    try {
      const session = await createSession({
        mode,
        name: f.name.trim() || null,
        participantIds: f.participantIds,
        songIds: f.songIds,
        order: f.order,
        minRounds: mode === "party" && f.minRounds !== "" ? Number(f.minRounds) : undefined,
        targetScore: mode === "challenge" ? Number(f.targetScore) : undefined,
      });
      replace({ name: "session", id: session.id });
    } catch (err) {
      toast(errorMessage(err), "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit} className="grid gap-6 xl:grid-cols-2">
      <Card className="flex flex-col gap-5">
        {(mode === "party" || mode === "challenge") && (
          <Input
            label={mode === "party" ? "Nome da festa" : "Nome do desafio (opcional)"}
            value={f.name}
            maxLength={60}
            onChange={(e) => setF({ ...f, name: e.currentTarget.value })}
          />
        )}
        <fieldset className="flex flex-col gap-3">
          <legend className="mb-2 font-semibold">
            Cantores <span className="font-normal text-muted">({rangeText(r.singers, "cantor", "cantores")})</span>
          </legend>
          {people.length === 0 && <p className="text-muted">Cadastre os cantores abaixo.</p>}
          <div className="flex flex-wrap gap-2">
            {people.map((p) => {
              const i = f.participantIds.indexOf(p.id);
              return (
                <Button
                  key={p.id}
                  variant={i >= 0 ? "primary" : "secondary"}
                  aria-pressed={i >= 0}
                  onClick={() => setF({ ...f, participantIds: toggle(f.participantIds, p.id, r.singers[1]) })}
                >
                  {i >= 0 && f.order === "manual" && r.singers[1] > 1 ? `${i + 1}. ` : ""}
                  {p.name}
                </Button>
              );
            })}
          </div>
          <AddParticipant
            onCreated={(p) => {
              setPeople((xs) => [...xs, p]);
              setF((cur) => ({ ...cur, participantIds: toggle(cur.participantIds, p.id, r.singers[1]) }));
            }}
          />
        </fieldset>

        {mode === "party" && (
          <>
            <fieldset className="flex flex-col gap-2">
              <legend className="mb-2 font-semibold">Ordem das rodadas</legend>
              {(
                [
                  ["shuffle", "Sorteada (reproduzível)"],
                  ["manual", "Na ordem em que escolhi cantores e músicas"],
                ] as const
              ).map(([value, label]) => (
                <label key={value} className="flex min-h-12 cursor-pointer items-center gap-3">
                  <input
                    type="radio"
                    name="order"
                    value={value}
                    checked={f.order === value}
                    onChange={() => setF({ ...f, order: value })}
                    className="size-6 accent-(--color-accent)"
                  />
                  {label}
                </label>
              ))}
            </fieldset>
            <Input
              label="Mínimo de rodadas para entrar no ranking"
              hint="Vazio = o menor número de rodadas atribuído a um cantor."
              inputMode="numeric"
              value={f.minRounds}
              onChange={(e) => setF({ ...f, minRounds: e.currentTarget.value })}
            />
          </>
        )}
        {mode === "challenge" && (
          <Input
            label="Meta de pontuação média (0–100)"
            inputMode="decimal"
            value={f.targetScore}
            onChange={(e) => setF({ ...f, targetScore: e.currentTarget.value })}
          />
        )}
      </Card>

      <Card className="flex flex-col gap-4">
        <fieldset className="flex flex-col gap-3">
          <legend className="mb-2 font-semibold">
            Músicas <span className="font-normal text-muted">({rangeText(r.songs, "música", "músicas")})</span>
          </legend>
          {r.validatedOnly && blocked > 0 && (
            <p className="text-sm text-muted">
              {blocked === 1
                ? "1 música não aparece porque ainda não tem referência validada"
                : `${blocked} músicas não aparecem porque ainda não têm referência validada`}
              : sem ela a pontuação não é justa para competir. Valide em Biblioteca → Referência.
            </p>
          )}
          <Input label="Buscar músicas" type="search" value={query} onChange={(e) => setQuery(e.currentTarget.value)} />
          {visibleSongs.length === 0 ? (
            <p className="text-muted">Nenhuma música disponível para este modo.</p>
          ) : (
            <ul className="flex max-h-[28rem] flex-col gap-1 overflow-y-auto pr-1">
              {visibleSongs.map((s) => {
                const i = f.songIds.indexOf(s.id);
                return (
                  <li key={s.id}>
                    <label className="flex min-h-12 cursor-pointer items-center gap-3 rounded-md px-3 hover:bg-surface-2">
                      <input
                        type={r.songs[1] === 1 ? "radio" : "checkbox"}
                        name="songs"
                        checked={i >= 0}
                        onChange={() => setF({ ...f, songIds: toggle(f.songIds, s.id, r.songs[1]) })}
                        className="size-6 accent-(--color-accent)"
                      />
                      <span className="flex-1">
                        {s.title}
                        {s.artist && <span className="text-muted"> · {s.artist}</span>}
                      </span>
                      {i >= 0 && f.order === "manual" && r.songs[1] > 1 && <Badge tone="accent">{i + 1}º</Badge>}
                    </label>
                  </li>
                );
              })}
            </ul>
          )}
        </fieldset>
      </Card>

      <div className="flex flex-col items-start gap-3 xl:col-span-2">
        {errors.length > 0 && (
          <ul aria-label="O que falta" className="list-inside list-disc text-warning">
            {errors.map((e) => (
              <li key={e}>{e}</li>
            ))}
          </ul>
        )}
        <Button type="submit" variant="primary" size="lg" disabled={busy || errors.length > 0}>
          Criar {modeLabel[mode].toLowerCase()}
        </Button>
      </div>
    </form>
  );
}
