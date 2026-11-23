// Karaoke-specific components (SPEC-010 T-010-02). Pure presentation: data comes from ipc.ts types.
import type { Difficulty, FeedbackLabel, GameMode, GameSession, LeaderboardRow, LiveFeedback, MicFrame, Standing } from "../lib/ipc";
import { Badge, type Tone } from "./ui";

const fmt1 = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 });
const fmt0 = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 0 });
export const formatScore = (n: number) => fmt1.format(n);

// Solfège, as Brazilian singers read it (coordinator decision): octave numbers follow scientific pitch (Dó4 = middle C).
const NOTE_NAMES = ["Dó", "Dó#", "Ré", "Ré#", "Mi", "Fá", "Fá#", "Sol", "Sol#", "Lá", "Lá#", "Si"];
/** MIDI note number → solfège name with octave (60 → "Dó4", 69 → "Lá4"). */
export function midiToName(midi: number): string {
  const n = Math.round(midi);
  return `${NOTE_NAMES[((n % 12) + 12) % 12]}${Math.floor(n / 12) - 1}`;
}

/** Frequency → fractional MIDI note (440 Hz = 69). */
export const hzToMidi = (hz: number) => 69 + 12 * Math.log2(hz / 440);

// ---------- MicMeter ----------
const METER_MIN_DB = -60;
/** Live input level. `noiseFloorDbfs` draws the calibrated gate line. */
export function MicMeter({ frame, noiseFloorDbfs }: { frame: MicFrame | null; noiseFloorDbfs?: number | null }) {
  const db = frame ? Math.max(METER_MIN_DB, Math.min(0, frame.rmsDbfs)) : METER_MIN_DB;
  const ratio = (db - METER_MIN_DB) / -METER_MIN_DB;
  const floorRatio =
    noiseFloorDbfs == null ? null : (Math.max(METER_MIN_DB, Math.min(0, noiseFloorDbfs)) - METER_MIN_DB) / -METER_MIN_DB;
  const valueText = frame ? `${fmt0.format(frame.rmsDbfs)} dBFS${frame.clipped ? ", saturando" : ""}` : "sem sinal";
  return (
    <div className="flex flex-col gap-2">
      <div
        role="meter"
        aria-label="Nível do microfone"
        aria-valuemin={METER_MIN_DB}
        aria-valuemax={0}
        aria-valuenow={Math.round(db)}
        aria-valuetext={valueText}
        className="relative h-4 overflow-hidden rounded-sm bg-surface-3"
      >
        <div
          className={`h-full origin-left transition-transform duration-(--duration-fast) ease-out ${frame?.clipped ? "bg-danger" : frame?.voiced ? "bg-success" : "bg-accent"}`}
          style={{ transform: `scaleX(${ratio})` }}
        />
        {floorRatio != null && (
          <div aria-hidden className="absolute inset-y-0 w-0.5 bg-fg/70" style={{ left: `${floorRatio * 100}%` }} />
        )}
      </div>
      <div className="flex flex-wrap items-center gap-2 text-sm text-muted">
        <span className="tabular-nums">{valueText}</span>
        {frame?.clipped && <Badge tone="danger">Saturando — afaste o microfone</Badge>}
        {frame?.voiced && <Badge tone="success">Voz detectada</Badge>}
        {frame?.voiced && frame.f0Hz != null && (
          <span className="tabular-nums">
            Nota {midiToName(hzToMidi(frame.f0Hz))} · {fmt0.format(frame.f0Hz)} Hz
          </span>
        )}
      </div>
    </div>
  );
}

// ---------- PitchIndicator ----------
const feedbackText: Record<FeedbackLabel, string> = {
  excellent: "Excelente",
  veryGood: "Muito bom",
  good: "Bom",
  needsWork: "Precisa melhorar",
  notDetected: "Não detectado",
};
const feedbackTone: Record<FeedbackLabel, Tone> = {
  excellent: "success",
  veryGood: "success",
  good: "info",
  needsWork: "warning",
  notDetected: "neutral",
};
const CENTS_RANGE = 100;
/** Sung pitch vs target, ±100 cents track. No live region: updates ~100×/s. */
export function PitchIndicator({ feedback }: { feedback: LiveFeedback | null }) {
  if (!feedback) {
    return <p className="text-muted">Aguardando a próxima nota…</p>;
  }
  const cents = feedback.centsError;
  const pos = cents == null ? null : Math.max(-1, Math.min(1, cents / CENTS_RANGE));
  const description =
    cents == null
      ? `Alvo ${midiToName(feedback.targetMidi)}, voz não detectada`
      : `Alvo ${midiToName(feedback.targetMidi)}, ${cents > 0 ? "acima" : cents < 0 ? "abaixo" : "no tom"}${cents !== 0 ? ` ${fmt0.format(Math.abs(cents))} cents` : ""}`;
  return (
    <div role="group" aria-label={description} className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <span className="font-display text-lg">{midiToName(feedback.targetMidi)}</span>
        <Badge tone={feedbackTone[feedback.label]}>{feedbackText[feedback.label]}</Badge>
      </div>
      <div aria-hidden className="@container relative h-3 rounded-sm bg-surface-3">
        <div className="absolute inset-y-0 left-1/2 w-0.5 bg-fg/60" />
        {pos != null && (
          <div
            className="absolute top-1/2 left-1/2 size-5 rounded-full bg-accent shadow-glow transition-transform duration-(--duration-fast) ease-out"
            style={{ transform: `translate(calc(${pos * 50}cqw - 50%), -50%)` }}
          />
        )}
      </div>
    </div>
  );
}

// ---------- ScoreBadge ----------
export function ScoreBadge({ score, competitive = true, size = "md" }: { score: number; competitive?: boolean; size?: "md" | "xl" }) {
  return (
    <div className="inline-flex flex-col items-center gap-1">
      <span
        aria-label={`${formatScore(score)} pontos${competitive ? "" : ", treino"}`}
        className={`bg-(image:--gradient-accent) bg-clip-text font-display font-bold text-transparent tabular-nums ${size === "xl" ? "text-8xl" : "text-4xl"}`}
      >
        {formatScore(score)}
      </span>
      {!competitive && <Badge tone="warning">Treino</Badge>}
    </div>
  );
}

// ---------- RankingTable ----------
const dateFmt = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short" });
export function RankingTable({ rows, caption, highlightId }: { rows: LeaderboardRow[]; caption: string; highlightId?: number }) {
  if (rows.length === 0) {
    return <p className="py-8 text-center text-muted">Ninguém pontuou nesta música ainda. Seja o primeiro!</p>;
  }
  return (
    <table className="w-full border-separate border-spacing-y-1 text-left">
      <caption className="sr-only">{caption}</caption>
      <thead className="text-sm text-muted">
        <tr>
          <th scope="col" className="px-4 py-2">#</th>
          <th scope="col" className="px-4 py-2">Cantor</th>
          <th scope="col" className="px-4 py-2 text-right">Pontos</th>
          <th scope="col" className="px-4 py-2 text-right">Data</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr
            key={r.performanceId}
            aria-current={r.performanceId === highlightId ? "true" : undefined}
            className="bg-surface-2 aria-[current=true]:bg-accent/15"
          >
            <td className="rounded-l-md px-4 py-3 font-display font-bold tabular-nums">{r.rank}</td>
            <th scope="row" className="px-4 py-3 font-medium">{r.participantName}</th>
            <td className="px-4 py-3 text-right font-semibold tabular-nums">{formatScore(r.score)}</td>
            <td className="rounded-r-md px-4 py-3 text-right text-muted tabular-nums">{dateFmt.format(r.createdMs)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

// ---------- Game modes / StandingsTable (SPEC-008) ----------
export const modeLabel: Record<GameMode, string> = { solo: "Solo", party: "Festa", duel: "Duelo", challenge: "Desafio" };
export const difficultyLabel: Record<Difficulty, string> = { easy: "Fácil", normal: "Normal", hard: "Difícil" };
export const difficultyOptions = Object.entries(difficultyLabel).map(([value, label]) => ({ value, label }));
export const sessionStatusLabel: Record<GameSession["status"], string> = { active: "Em andamento", completed: "Encerrada", abandoned: "Abandonada" };
/** Winner of a finished party: best eligible average (rank 1). */
export const partyWinner = (s: GameSession) => (s.status === "completed" ? s.standings.find((x) => x.eligible && x.rank === 1) : undefined);
export const roundsLeft = (n: number) => (n === 1 ? "falta 1 rodada" : `faltam ${n} rodadas`);
export const formatMB = (bytes: number) => `${fmt1.format(bytes / 1_048_576)} MB`;

/** Party standings: ranked by AVERAGE among eligible (rounds ≥ minRounds); ineligible listed below, flagged. */
export function StandingsTable({
  standings,
  names,
  minRounds,
  caption,
}: {
  standings: Standing[];
  names: Map<number, string>;
  minRounds: number | undefined;
  caption: string;
}) {
  if (standings.length === 0) return <p className="py-6 text-center text-muted">Ninguém cantou ainda.</p>;
  const rows = [...standings].sort((a, b) => Number(b.eligible) - Number(a.eligible) || a.rank - b.rank);
  return (
    <table className="w-full border-separate border-spacing-y-1 text-left">
      <caption className="sr-only">{caption}</caption>
      <thead className="text-sm text-muted">
        <tr>
          <th scope="col" className="px-4 py-2">#</th>
          <th scope="col" className="px-4 py-2">Cantor</th>
          <th scope="col" className="px-4 py-2 text-right">Média</th>
          <th scope="col" className="px-4 py-2 text-right">Total</th>
          <th scope="col" className="px-4 py-2 text-right">Rodadas</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr key={r.participantId} className={r.eligible ? "bg-surface-2" : "bg-surface-2/50 text-muted"}>
            <td className="rounded-l-md px-4 py-3 font-display font-bold tabular-nums">{r.eligible ? r.rank : "–"}</td>
            <th scope="row" className="px-4 py-3 font-medium">
              <span className="flex flex-wrap items-center gap-2">
                {names.get(r.participantId) ?? `#${r.participantId}`}
                {!r.eligible && (
                  <Badge tone="warning">
                    {minRounds != null && r.rounds < minRounds
                      ? `Inelegível: ${roundsLeft(minRounds - r.rounds)}`
                      : "Inelegível"}
                  </Badge>
                )}
              </span>
            </th>
            <td className="px-4 py-3 text-right font-semibold tabular-nums">{r.rounds ? formatScore(r.average) : "–"}</td>
            <td className="px-4 py-3 text-right tabular-nums">{formatScore(r.total)}</td>
            <td className="rounded-r-md px-4 py-3 text-right tabular-nums">{r.rounds}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
