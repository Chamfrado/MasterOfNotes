import { ScoreBadge, formatScore } from "../components/game";
import { Button, Card } from "../components/ui";
import type { PerformanceOutcome } from "../lib/ipc";
import { useNav, type RoundContext } from "../nav";

const percent = new Intl.NumberFormat("pt-BR", { style: "percent", maximumFractionDigits: 0 });

export function Result({ songId, outcome, round }: { songId: string; outcome: PerformanceOutcome; round?: RoundContext }) {
  const { navigate, replace, back } = useNav();
  const r = outcome.result;
  return (
    <div className="flex flex-col items-center gap-10 pt-8 text-center">
      <h1 className="font-display text-5xl font-bold">Resultado</h1>
      {r ? (
        <>
          <div className="animate-enter">
            <ScoreBadge score={r.score} competitive={outcome.competitive} size="xl" />
          </div>
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
                <dd className="font-display text-3xl font-bold tabular-nums">{formatScore(v)}</dd>
              </Card>
            ))}
          </dl>
          <p className="text-muted">
            {r.notesHit} de {r.notesTotal} notas acertadas
            {outcome.songRecord != null && ` · recorde da música: ${formatScore(outcome.songRecord)}`}
          </p>
        </>
      ) : (
        <Card className="flex max-w-xl flex-col gap-4">
          <p>Esta música ainda não tem referência melódica, então não há pontuação. Importe uma referência para competir.</p>
          <Button className="self-center" onClick={() => navigate({ name: "reference", songId })}>
            Importar referência
          </Button>
          {outcome.exploratory && (
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
          )}
        </Card>
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
