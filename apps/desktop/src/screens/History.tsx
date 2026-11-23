import { modeLabel, partyWinner, sessionStatusLabel } from "../components/game";
import { Badge, ErrorState, LoadingState } from "../components/ui";
import { listParticipants, listSessions } from "../lib/ipc";
import { useNav } from "../nav";
import { useAsync } from "../useAsync";

const dateFmt = new Intl.DateTimeFormat("pt-BR", { dateStyle: "medium", timeStyle: "short" });

export function History() {
  const { navigate } = useNav();
  const [load, reload] = useAsync(() => Promise.all([listSessions(), listParticipants()]), []);
  return (
    <div className="flex flex-col gap-8">
      <h1 className="font-display text-5xl font-bold">Histórico de partidas</h1>
      {load.status === "loading" && <LoadingState />}
      {load.status === "error" && <ErrorState title="Não foi possível carregar o histórico" error={load.error} onRetry={reload} />}
      {load.status === "ready" &&
        (() => {
          const names = new Map(load.data[1].map((p) => [p.id, p.name]));
          const sessions = [...load.data[0]].sort((a, b) => b.startedMs - a.startedMs);
          if (sessions.length === 0) return <p className="text-muted">Nenhuma partida ainda. Que tal começar uma festa?</p>;
          return (
            <ul className="flex flex-col gap-2">
              {sessions.map((s) => {
                const w = partyWinner(s);
                return (
                  <li key={s.id}>
                    <button
                      type="button"
                      onClick={() => navigate({ name: "session", id: s.id })}
                      className="flex min-h-12 w-full cursor-pointer flex-wrap items-center gap-4 rounded-md border border-line bg-surface px-5 py-3 text-left hover:bg-surface-2"
                    >
                      <span className="flex-1">
                        <strong>{s.name ?? modeLabel[s.mode]}</strong>{" "}
                        <span className="text-muted">
                          · {modeLabel[s.mode]} · {dateFmt.format(s.startedMs)}
                        </span>
                      </span>
                      {w && <span>Vencedor: {names.get(w.participantId) ?? `#${w.participantId}`}</span>}
                      <Badge tone={s.status === "active" ? "accent" : "neutral"}>{sessionStatusLabel[s.status]}</Badge>
                    </button>
                  </li>
                );
              })}
            </ul>
          );
        })()}
    </div>
  );
}
