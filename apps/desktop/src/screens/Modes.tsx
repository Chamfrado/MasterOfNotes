import { modeLabel, roundsLeft } from "../components/game";
import { Button, Card, ErrorState } from "../components/ui";
import { listSessions, type GameMode } from "../lib/ipc";
import { useNav } from "../nav";
import { useAsync } from "../useAsync";

const modes: { mode: GameMode; text: string }[] = [
  { mode: "solo", text: "Uma música, sua nota e seus recordes." },
  { mode: "party", text: "Várias pessoas se revezam; vence a melhor média." },
  { mode: "duel", text: "Duas vozes, a mesma música, um vencedor." },
  { mode: "challenge", text: "Uma sequência de músicas e uma meta de média." },
];

export function Modes() {
  const { navigate } = useNav();
  const [sessions, reload] = useAsync(listSessions, []);
  const active = sessions.status === "ready" ? sessions.data.filter((s) => s.status === "active") : [];
  return (
    <div className="flex flex-col gap-10">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <h1 className="font-display text-5xl font-bold">Jogar</h1>
        <div className="flex gap-3">
          <Button onClick={() => navigate({ name: "participants" })}>Participantes</Button>
          <Button onClick={() => navigate({ name: "history" })}>Histórico</Button>
        </div>
      </header>

      {sessions.status === "error" && <ErrorState title="Não foi possível ver as partidas em andamento" error={sessions.error} onRetry={reload} />}
      {active.length > 0 && (
        <section aria-labelledby="resume-title" className="flex flex-col gap-3">
          <h2 id="resume-title" className="font-display text-2xl font-semibold">
            Continuar de onde parou
          </h2>
          <ul className="flex flex-col gap-2">
            {active.map((s) => {
              const left = s.rounds.filter((r) => r.status === "pending").length;
              return (
                <li key={s.id}>
                  <Card className="flex flex-wrap items-center justify-between gap-4 py-4">
                    <span>
                      <strong>{s.name ?? modeLabel[s.mode]}</strong>{" "}
                      <span className="text-muted">
                        · {modeLabel[s.mode]} · {roundsLeft(left)}
                      </span>
                    </span>
                    <Button variant="primary" onClick={() => navigate({ name: "session", id: s.id })}>
                      Retomar
                    </Button>
                  </Card>
                </li>
              );
            })}
          </ul>
        </section>
      )}

      <ul className="grid gap-4 md:grid-cols-2">
        {modes.map(({ mode, text }) => (
          <li key={mode}>
            <button
              type="button"
              onClick={() => navigate({ name: "new-session", mode })}
              className="flex min-h-40 w-full cursor-pointer flex-col items-start justify-end gap-2 rounded-xl border border-line bg-surface p-8 text-left shadow-card transition-transform duration-(--duration-base) ease-out hover:-translate-y-1 focus-visible:-translate-y-1"
            >
              <span className="font-display text-4xl font-bold">{modeLabel[mode]}</span>
              <span className="text-muted">{text}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
