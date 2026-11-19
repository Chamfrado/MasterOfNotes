import { ScoreBadge, formatScore } from "../components/game";
import { Badge, Button, Card } from "../components/ui";
import { getSettings, listSongs, type AppSettings, type PerformanceOutcome, type Song } from "../lib/ipc";
import { useNav, type Route, type RoundContext } from "../nav";
import { leakCheckState } from "../player/leak";
import { useAsync } from "../useAsync";

const percent = new Intl.NumberFormat("pt-BR", { style: "percent", maximumFractionDigits: 0 });

export interface TrainingAction {
  label: string;
  to: Route;
}
/**
 * T-006-09: the shortcut that fixes why a run was practice, derived from state (never from the reason text).
 * Seek, audio dropouts or too little played have no shortcut: singing again is the fix.
 */
export function trainingAction(song: Pick<Song, "id" | "referenceStatus">, settings: Pick<AppSettings, "leakCheck" | "inputDeviceId" | "videoVolume">): TrainingAction | null {
  if (song.referenceStatus === "none") return { label: "Importar referência", to: { name: "reference", songId: song.id } };
  if (song.referenceStatus !== "validated") return { label: "Abrir referência", to: { name: "reference", songId: song.id } };
  if (leakCheckState(settings) !== "passed") return { label: "Fazer teste de vazamento", to: { name: "mic" } };
  return null;
}

/** Why this run is out of the ranking (backend reason) + the fix, when there is one. */
function TrainingNotice({ songId, reason }: { songId: string; reason: string | null | undefined }) {
  const { navigate } = useNav();
  const [state] = useAsync(() => Promise.all([listSongs(), getSettings()]), [songId]);
  const song = state.status === "ready" ? state.data[0].find((s) => s.id === songId) : undefined;
  const action = song && state.status === "ready" ? trainingAction(song, state.data[1]) : null;
  return (
    <Card className="flex max-w-2xl flex-col items-center gap-3 border-warning/50 text-center">
      <Badge tone="warning">Treino — fora do ranking</Badge>
      <p className="text-lg">{reason ?? "Esta apresentação não valeu para o ranking."}</p>
      {action && (
        <Button variant="primary" onClick={() => navigate(action.to)}>
          {action.label}
        </Button>
      )}
    </Card>
  );
}

export function Result({ songId, outcome, round }: { songId: string; outcome: PerformanceOutcome; round?: RoundContext }) {
  const { navigate, replace, back } = useNav();
  const r = outcome.result;
  return (
    <div className="flex flex-col items-center gap-10 pt-8 text-center">
      <h1 className="font-display text-5xl font-bold">Resultado</h1>
      {r ? (
        <>
          <div className="animate-enter">
            {/* The practice flag is spelled out in the notice below, so the badge itself stays clean. */}
            <ScoreBadge score={r.score} size="xl" />
          </div>
          {!outcome.competitive && <TrainingNotice songId={songId} reason={outcome.reason} />}
          {outcome.personalBest != null && r.score >= outcome.personalBest && (
            <p className="text-xl font-semibold text-success">Novo recorde pessoal!</p>
          )}
          <dl className="grid w-full max-w-3xl grid-cols-2 gap-4 md:grid-cols-4">
            {(
              [
                ["Afinação", r.pitch],
                ["Ritmo", r.rhythm],
                ["Sustentação", r.sustain],
                ["Cobertura", r.coverage],
              ] as const
            ).map(([label, v]) => (
              <Card key={label} className="flex flex-col gap-1 p-4">
                <dt className="text-sm text-muted">{label}</dt>
                <dd className="font-display text-3xl font-bold tabular-nums">{percent.format(v)}</dd>{/* components are 0..1 (scorer.rs) */}
              </Card>
            ))}
          </dl>
          <p className="text-muted">
            {r.notesHit} de {r.notesTotal} notas acertadas
            {outcome.songRecord != null && ` · recorde da música: ${formatScore(outcome.songRecord)}`}
          </p>
        </>
      ) : (
        <>
          <TrainingNotice songId={songId} reason={outcome.reason} />
          {outcome.exploratory && (
            <Card className="flex max-w-xl flex-col gap-4">
              <section aria-labelledby="exploratory-title" className="flex flex-col gap-3">
                <h2 id="exploratory-title" className="font-semibold">
                  Métricas exploratórias <span className="text-muted">(fora do ranking)</span>
                </h2>
                <dl className="grid grid-cols-2 gap-4">
                  {(
                    [
                      ["Tempo cantando", outcome.exploratory.voicedRatio],
                      ["Estabilidade da afinação", outcome.exploratory.pitchStability],
                    ] as const
                  ).map(([label, v]) => (
                    <div key={label} className="flex flex-col gap-1 rounded-md bg-surface-2 p-4">
                      <dt className="text-sm text-muted">{label}</dt>
                      <dd className="font-display text-3xl font-bold tabular-nums">{percent.format(v)}</dd>
                    </div>
                  ))}
                </dl>
              </section>
            </Card>
          )}
        </>
      )}
      <div className="flex flex-wrap justify-center gap-4">
        {round ? (
          // The round is settled by Rust (completed only if competitive). Player was replaced by
          // this screen, so the previous one is the session; it reloads on mount.
          <Button variant="primary" size="lg" autoFocus onClick={back}>
            Voltar para a partida
          </Button>
        ) : (
          <Button variant="primary" size="lg" onClick={() => replace({ name: "player", songId })}>
            Cantar de novo
          </Button>
        )}
        <Button size="lg" onClick={() => navigate({ name: "ranking", songId })}>
          Ver ranking
        </Button>
        <Button size="lg" variant="ghost" onClick={() => navigate({ name: "library" })}>
          Outra música
        </Button>
      </div>
    </div>
  );
}
