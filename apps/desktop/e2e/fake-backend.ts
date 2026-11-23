// Test-only stateful fake of the session/performance/reference commands (Law 03: never imported by src/).
// The real rules live in Rust (crates/game, src-tauri/sessions.rs) and are tested there; this fake only
// mirrors their observable shape so UI flows can be driven end to end. State survives page reloads
// (localStorage), which stands in for closing and reopening the app.
import type { Page } from "@playwright/test";
import type {
  AlignProposal,
  AppSettings,
  AutoReferenceOutcome,
  AutoReferenceProgress,
  GameSession,
  InstalledSeparationModel,
  KnownSeparationModel,
  MelodyReference,
  Participant,
  PerformanceOutcome,
  RefNote,
  Round,
  Standing,
  StartPerformance,
} from "../src/lib/ipc";

/** Invented entry (Law 19) standing in for song_analysis::known_models(). */
export const knownModel: KnownSeparationModel = {
  id: "5e1f".padEnd(64, "0"),
  name: "Separador Exemplo v1",
  fileName: "separador-exemplo-v1.onnx",
  sizeBytes: 66_000_000,
  pageUrl: "https://example.org/separador-exemplo",
  sourceUrl: "https://example.org/separador-exemplo/separador-exemplo-v1.onnx",
  license: "Uso pessoal e não comercial; confira os termos na fonte.",
};
/** SPEC-012 R3. */
export const NO_VOICE_REASON =
  "Não encontramos uma voz clara neste vídeo: pode ser só instrumental ou a voz estar baixa demais. Use Gravar cantando ou importe um MIDI.";
/** One generate_reference result in FakeOptions.generated. */
type Generated = RefNote[] | "noVoice" | { notes: RefNote[]; voiceLevelDb: number };
const generatedNotes: RefNote[] = [60, 62, 64, 65, 67].map((midi, i) => ({ startMs: 1000 + i * 500, durMs: 450, midi, lyric: null }));

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
  /** settings.separationModel (SPEC-012 R1). */
  separationModel: InstalledSeparationModel | null;
  generated: Generated[];
  alignments: AlignProposal[];
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
  /** song_analysis::known_models() stand-in (default [knownModel]). Install matches the picked file by NAME: the fake cannot hash. */
  knownModels?: KnownSeparationModel[];
  /** Model already installed when the app opens. */
  separationModel?: InstalledSeparationModel | null;
  /** What each generate_reference run finds, in order: the notes (voice at −6.2 dB), notes at a given voice level (e.g. weak voice,
   * between −20 and −10 dB), or "noVoice" (R3). None left: 5 notes. */
  generated?: Generated[];
  /** Pause between generate_reference progress messages (default 150 ms; 3 stages × 3 messages). */
  generateStepMs?: number;
  /** auto_align_reference proposals (absolute offsets), in order. None left: current offset + 1200 ms, confidence 0.9. */
  alignments?: AlignProposal[];
  /** UltraStar .txt files the "user" can pick, by file name (the fake does not parse: scoring_engine::import_ultrastar does).
   * A file not listed here is refused like an unreadable one. */
  ultrastar?: Record<string, UltraStarFile>;
}
/** What import_ultrastar gives for one file: its P1 notes (and whether it was a duet), or the invalid line. */
type UltraStarFile = { notes: RefNote[]; duet?: boolean } | { line: number; message: string };
/** SPEC-013 R3 (MelodyReference.notice, import result only). */
export const DUET_NOTICE = "Arquivo de dueto: importamos a parte do primeiro cantor.";

export async function installFakeBackend(page: Page, scores: number[], options: FakeOptions = {}) {
  const opts = { knownModels: [knownModel], generateStepMs: 150, ...options };
  await page.addInitScript(({ scores, opts, NO_VOICE_REASON, generatedNotes, DUET_NOTICE }) => {
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
        separationModel: opts.separationModel ?? null,
        generated: opts.generated ?? [],
        alignments: opts.alignments ?? [],
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
    /** reference.rs store(): the new reference is the active one; the previous stays in the history (R5). */
    const storeRef = (db: Db, songId: string, source: MelodyReference["source"], notes: RefNote[]): MelodyReference => {
      const r: MelodyReference = { id: db.nextId++, songId, source, status: "experimental", revision: 1, offsetMs: 0, notes };
      db.references.push(r);
      db.activeRefs[songId] = r.id;
      return r;
    };
    /** The static spec's reply (setReply works), for commands the fake only extends. */
    const fromSpec = <T,>(cmd: string): T | undefined => {
      const r = window.__E2E_SPEC__?.[cmd];
      if (r && "err" in r) throw r.err;
      return r?.ok as T | undefined;
    };
    // In memory only, like the Rust side: the uploaded audio (1 item) and the running generate job.
    let audio: { token: string; songId: string } | null = null;
    let uploads = 0;
    let job: { cancelled: boolean } | null = null;
    const needAudio = (token: unknown, songId: string) => {
      if (!audio || audio.token !== token || audio.songId !== songId) fail("O áudio desta música não está carregado. Tente de novo.", "audio_token_invalid");
    };
    const runCallback = (id: number, data: unknown) =>
      (window as unknown as { __TAURI_INTERNALS__: { runCallback: (id: number, data: unknown) => void } }).__TAURI_INTERNALS__.runCallback(id, data);
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

    const handlers: NonNullable<Window["__E2E_HANDLERS__"]> = {
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
          const r = db.references.find((x) => x.id === referenceId) ?? fail("Referência não encontrada.", "not_found");
          r.status = status as MelodyReference["status"];
          return r;
        }),
      set_reference_offset: ({ referenceId, offsetMs }) =>
        tx((db) => {
          const r = db.references.find((x) => x.id === referenceId) ?? fail("Referência não encontrada.", "not_found");
          const o = Number(offsetMs);
          if (!(Math.abs(o) <= 10_000)) fail("O ajuste deve estar entre -10 e +10 segundos.", "invalid_input");
          if (o !== r.offsetMs) Object.assign(r, { offsetMs: o, revision: r.revision + 1, status: "experimental" }); // R4
          return r;
        }),

      // ---------- SPEC-012: separation model, song audio, automatic reference and alignment ----------
      get_settings: () => ({ ...fromSpec<AppSettings>("get_settings"), separationModel: load().separationModel }),
      update_settings: () => ({ ...fromSpec<AppSettings>("update_settings"), separationModel: load().separationModel }),
      list_known_separation_models: () => opts.knownModels,
      install_separation_model: ({ filePath, acceptTerms }) =>
        tx((db) => {
          if (acceptTerms !== true) fail('Marque "Li e aceito os termos da fonte" para instalar o modelo.', "terms_not_accepted");
          const name = String(filePath).split(/[\\/]/).pop() ?? "";
          const m =
            opts.knownModels.find((k) => k.fileName === name) ??
            fail(`"${name}" não é um modelo conhecido: só os modelos da lista são aceitos.`, "model_unknown");
          db.separationModel = { id: m.id, name: m.name, acceptedMs: Date.UTC(2026, 10, 20, 12, 0) };
          return db.separationModel;
        }),
      remove_separation_model: () => tx((db) => void (db.separationModel = null)),
      // Raw IPC: the body is the f32 LE mono 44.1 kHz audio; the song comes in the "song-id" header.
      upload_song_audio: (body, options) => {
        const songId = options?.headers?.["song-id"] ?? fail("Falta o cabeçalho song-id.", "audio_invalid");
        const bytes: unknown = body;
        if (!(bytes instanceof Uint8Array) || bytes.byteLength === 0 || bytes.byteLength % 4 !== 0)
          fail("Áudio inválido: esperado f32 mono 44,1 kHz.", "audio_invalid");
        audio = { token: `audio-${++uploads}`, songId };
        return audio.token;
      },
      generate_reference: async ({ songId, audioToken, onProgress }): Promise<AutoReferenceOutcome> => {
        if (!load().separationModel) fail("Instale um modelo de separação em Ajustes → Referência automática.", "model_not_installed");
        needAudio(audioToken, String(songId));
        if (job) fail("Já existe uma referência sendo gerada. Aguarde ou cancele.", "busy");
        const me = (job = { cancelled: false });
        const started = Date.now();
        const next = load().generated[0] ?? generatedNotes;
        const plan = Array.isArray(next) || next === "noVoice" ? { notes: next, voiceLevelDb: -6.2 } : next;
        let index = 0;
        const stage = async (s: AutoReferenceProgress["stage"]) => {
          for (const fraction of [0, 0.5, 1]) {
            if (me.cancelled) fail("Geração cancelada.", "cancelled");
            runCallback((onProgress as { id: number }).id, { index: index++, message: { stage: s, fraction } });
            await new Promise((resolve) => setTimeout(resolve, opts.generateStepMs));
          }
          if (me.cancelled) fail("Geração cancelada.", "cancelled");
        };
        try {
          await stage("separating");
          if (plan.notes === "noVoice")
            return tx((db) => {
              db.generated.shift();
              return { reference: null, reason: NO_VOICE_REASON, voiceLevelDb: -41.5, elapsedMs: Date.now() - started };
            });
          await stage("transcribing");
          await stage("saving");
          return tx((db) => {
            db.generated.shift();
            return { reference: storeRef(db, String(songId), "extracted", plan.notes as RefNote[]), reason: null, voiceLevelDb: plan.voiceLevelDb, elapsedMs: Date.now() - started };
          });
        } finally {
          if (job === me) job = null;
        }
      },
      // The slot is free when cancel returns (a new "Começar" right after it must not get `busy`); the old job
      // still rejects with `cancelled` at its next step. ❓ Rust (T-012-03) must behave the same.
      cancel_generate_reference: () => {
        if (job) job.cancelled = true;
        job = null;
        return null;
      },
      // Proposes only; the UI saves with set_reference_offset if the user accepts (R5).
      auto_align_reference: ({ referenceId, audioToken }) =>
        tx((db) => {
          const r = db.references.find((x) => x.id === referenceId) ?? fail("Referência não encontrada.", "not_found");
          needAudio(audioToken, r.songId);
          return db.alignments.shift() ?? { offsetMs: r.offsetMs + 1200, confidence: 0.9 };
        }),

      // SPEC-013: .txt = UltraStar (source "ultrastar", duet → notice); MIDI/JSON keep the static spec reply.
      import_reference: ({ songId, filePath }) => {
        const name = String(filePath).split(/[\\/]/).pop() ?? "";
        if (!/\.txt$/i.test(name)) return fromSpec<MelodyReference>("import_reference") ?? fail("Comando não mockado no teste: import_reference", "E2E_UNMOCKED");
        const file = opts.ultrastar?.[name] ?? fail(`Não foi possível ler o arquivo: ${name}.`, "not_found");
        if ("line" in file) return fail(`Referência inválida: arquivo UltraStar, linha ${file.line}: ${file.message}.` /* ref_err + RefError Display */, "invalid_reference");
        return tx((db): MelodyReference => ({
          ...storeRef(db, String(songId), "ultrastar", file.notes),
          ...(file.duet ? { notice: DUET_NOTICE } : {}), // omitted (skip) when there is no notice, like the Rust JSON
        }));
      },

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
            return { ...none, capturedReference: storeRef(db, songId, "manual", notes) };
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
  }, { scores, opts, NO_VOICE_REASON, generatedNotes, DUET_NOTICE });
}
