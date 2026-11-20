// Test-only stateful fake of the session/performance commands (Law 03: never imported by src/).
// The real rules live in Rust (crates/game, src-tauri/sessions.rs) and are tested there; this fake only
// mirrors their observable shape so UI flows can be driven end to end. State survives page reloads
// (localStorage), which stands in for closing and reopening the app.
import type { Page } from "@playwright/test";
import type { GameSession, MelodyReference, Participant, PerformanceOutcome, RefNote, Round, Standing, StartPerformance } from "../src/lib/ipc";

interface Db {
  participants: Participant[];
  sessions: GameSession[];
  /** Score given to each finished performance, in order. */
  scores: number[];
  active: { roundId: number | null; captureSongId?: string } | null;
  nextId: number;
  /** Every reference ever stored (history, R5); `activeRefs` maps song → its active one. */
  references: MelodyReference[];
  activeRefs: Record<string, number>;
  /** Notes "detected" by each sung reference capture, in order (none left = nothing detected). */
  captures: RefNote[][];
}

export interface FakeOptions {
  /** Singers that already exist when the app opens. */
  participants?: Participant[];
  /** The two duel performances have different CompatKeys (e.g. the reference got a new revision in between). */
  incompatibleDuel?: boolean;
  /** Active references when the app opens. */
  references?: MelodyReference[];
  /** Notes each `referenceCapture` run "detects", in order (live.rs keeps ≥ 3 as the new active reference). */
  captures?: RefNote[][];
}

export async function installFakeBackend(page: Page, scores: number[], opts: FakeOptions = {}) {
  await page.addInitScript(({ scores, opts }) => {
    const KEY = "e2e-fake-db";
    const load = (): Db =>
      JSON.parse(localStorage.getItem(KEY) ?? "null") ?? {
        participants: opts.participants ?? [],
        sessions: [],
        scores,
        active: null,
        nextId: 100,
        references: opts.references ?? [],
        activeRefs: Object.fromEntries((opts.references ?? []).map((r) => [r.songId, r.id])),
        captures: opts.captures ?? [],
      };
    const fail = (message: string, code = "TEST") => {
      throw { code, message };
    };
    /** Runs `fn` on the persisted db and saves it. */
    const tx = <T,>(fn: (db: Db) => T): T => {
      const db = load();
      const out = fn(db);
      localStorage.setItem(KEY, JSON.stringify(db));
      return out;
    };
    // Same verdict as game::duel_outcome (tenths; draw <=> margin 0), filled by get_session like sessions.rs.
    const withDuel = (s: GameSession): GameSession => {
      if (s.mode !== "duel") return s;
      const done = s.rounds.filter((r) => r.status === "completed" && r.score != null);
      if (done.length < 2) return { ...s, duel: null };
      if (opts.incompatibleDuel)
        return { ...s, duel: { kind: "incompatible", message: "as performances não são comparáveis (referência, revisão, algoritmo ou dificuldade diferentes)" } };
      const [a, b] = done.map((r) => Math.round((r.score ?? 0) * 10));
      const winner = a === b ? null : a > b ? done[0].participantId : done[1].participantId;
      return { ...s, duel: { kind: "result", winner, margin: Math.abs(a - b) / 10 } };
    };
    const session = (db: Db, id: unknown) => db.sessions.find((s) => s.id === id) ?? fail("Partida não encontrada.");

    // Same shape as game::party_standings + the glue that lists 0-round participants (contracts.md §2.3).
    const standings = (s: GameSession): Standing[] => {
      const ids = [...new Set(s.rounds.map((r) => r.participantId).filter((id): id is number => id != null))];
      const rows = ids.map((id) => {
        const done = s.rounds.filter((r) => r.participantId === id && r.status === "completed" && r.score != null);
        const total = done.reduce((t, r) => t + (r.score ?? 0), 0);
        const rounds = done.length;
        return { participantId: id, total, rounds, average: rounds ? total / rounds : 0, eligible: rounds >= (s.config.minRounds ?? 1), rank: 0 };
      });
      rows.sort((a, b) => Number(b.eligible) - Number(a.eligible) || b.average - a.average || b.total - a.total || a.participantId - b.participantId);
      rows.forEach((r, i) => (r.rank = i + 1));
      return rows;
    };

    const handlers: Record<string, (a: Record<string, unknown>) => unknown> = {
      list_participants: () => load().participants,
      create_participant: ({ name }) =>
        tx((db) => {
          const n = String(name).trim();
          if (!n) fail("Digite um nome.");
          if (db.participants.some((p) => p.name.toLowerCase() === n.toLowerCase())) fail(`Já existe um cantor chamado "${n}".`);
          const p = { id: db.nextId++, name: n };
          db.participants.push(p);
          return p;
        }),
      list_sessions: () => load().sessions.map(withDuel),
      get_session: ({ id }) => withDuel(session(load(), id)),
      create_session: ({ args }) =>
        tx((db) => {
          const a = args as { mode: GameSession["mode"]; name: string | null; participantIds: number[]; songIds: string[]; minRounds?: number; targetScore?: number };
          // Manual order, as plan_rounds: max(P, S) rounds, round i = (p[i % P], s[i % S]).
          const n = Math.max(a.participantIds.length, a.songIds.length);
          const rounds: Round[] = Array.from({ length: n }, (_, i) => ({
            id: db.nextId++,
            position: i + 1,
            songId: a.songIds[i % a.songIds.length],
            participantId: a.participantIds.length ? a.participantIds[i % a.participantIds.length] : null,
            status: "pending",
            score: null,
          }));
          const perSinger = a.participantIds.map((id) => rounds.filter((r) => r.participantId === id).length);
          const s: GameSession = {
            id: db.nextId++,
            mode: a.mode,
            name: a.name,
            status: "active",
            config: {
              minRounds: a.mode === "party" ? (a.minRounds ?? Math.min(...perSinger)) : undefined,
              targetScore: a.mode === "challenge" ? a.targetScore : undefined,
              difficulty: "normal",
              seed: 42,
            },
            startedMs: Date.UTC(2026, 10, 19, 20, 0),
            completedMs: null,
            rounds,
            standings: [],
          };
          s.standings = standings(s);
          db.sessions.push(s);
          return withDuel(s);
        }),
      set_round_status: ({ roundId, status }) =>
        tx((db) => {
          const s = db.sessions.find((x) => x.rounds.some((r) => r.id === roundId)) ?? fail("Rodada não encontrada.");
          const r = s.rounds.find((x) => x.id === roundId);
          if (r) r.status = status as Round["status"];
          return withDuel(s);
        }),
      complete_session: ({ id }) =>
        tx((db) => {
          const s = session(db, id);
          s.status = "completed";
          s.completedMs = Date.UTC(2026, 10, 19, 21, 0);
          return withDuel(s);
        }),
      // References (reference.rs): only the active one is visible; new ones deactivate the previous (R5).
      get_reference: ({ songId }) => {
        const db = load();
        return db.references.find((r) => r.id === db.activeRefs[String(songId)]) ?? null;
      },
      set_reference_status: ({ referenceId, status }) =>
        tx((db) => {
          const r = db.references.find((x) => x.id === referenceId) ?? fail("Referência não encontrada.");
          r.status = status as MelodyReference["status"];
          return r;
        }),
      start_performance: ({ args }) =>
        tx((db) => {
          const a = args as StartPerformance;
          // Same rules as live.rs start_performance (T-005-07): a capture runs alone and is never competitive.
          if (a.referenceCapture) {
            if (a.leakCheck || a.latencyProbe || a.roundId != null || a.participantId != null)
              fail("A gravação da referência roda sozinha: sem teste de vazamento, sonda, rodada ou participante.", "invalid_input");
            db.active = { roundId: null, captureSongId: a.songId };
            return { competitive: false, reason: "Gravação da referência: cante a melodia da música." };
          }
          db.active = { roundId: a.roundId };
          return { competitive: true, reason: null };
        }),
      sync_performance: () => null,
      abort_performance: () => tx((db) => void (db.active = null)),
      finish_performance: () =>
        tx((db): PerformanceOutcome => {
          const songId = db.active?.captureSongId;
          if (songId) {
            // live.rs finish: < 3 notes stores nothing; otherwise reference::store_captured.
            db.active = null;
            const notes = db.captures.shift() ?? [];
            const none = { performanceId: null, competitive: false, result: null, exploratory: null, personalBest: null, songRecord: null };
            if (notes.length < 3)
              return { ...none, capturedReference: null, reason: "Poucas notas detectadas: cante a melodia mais perto do microfone e tente de novo." };
            const r: MelodyReference = { id: db.nextId++, songId, source: "manual", status: "experimental", revision: 1, offsetMs: 0, notes };
            db.references.push(r);
            db.activeRefs[songId] = r.id;
            return { ...none, capturedReference: r };
          }
          const score = db.scores.shift() ?? 50;
          const roundId = db.active?.roundId;
          db.active = null;
          const s = db.sessions.find((x) => x.rounds.some((r) => r.id === roundId));
          const r = s?.rounds.find((x) => x.id === roundId);
          if (s && r) {
            r.status = "completed";
            r.score = score;
            s.standings = standings(s);
          }
          return {
            performanceId: db.nextId++,
            competitive: true,
            result: { score, pitch: score / 100, rhythm: score / 100, sustain: score / 100, coverage: 1, notesTotal: 10, notesHit: 10, algorithmVersion: 1 },
            exploratory: null,
            personalBest: null,
            songRecord: null,
          };
        }),
    };
    window.__E2E_HANDLERS__ = handlers;
  }, { scores, opts });
}
