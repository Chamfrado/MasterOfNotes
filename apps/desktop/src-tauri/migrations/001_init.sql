-- Migration 001. Source of truth: docs/architecture/contracts.md §3.
-- Never edit after integration; schema changes go in a new numbered file.
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);           -- value = JSON
CREATE TABLE songs (
  id TEXT PRIMARY KEY,               -- content id: blake3(size ‖ primeiros 64KiB ‖ últimos 64KiB), hex 32
  path TEXT NOT NULL UNIQUE,
  file_name TEXT NOT NULL,
  title TEXT NOT NULL,               -- inicial = nome do arquivo sem extensão
  artist TEXT,
  extension TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  modified_ms INTEGER NOT NULL,
  duration_ms INTEGER,               -- preenchido pela UI em loadedmetadata
  playable TEXT NOT NULL DEFAULT 'unknown' CHECK (playable IN ('unknown','yes','no')),
  available INTEGER NOT NULL DEFAULT 1,  -- 0 = arquivo sumiu; linha mantida (histórico)
  added_ms INTEGER NOT NULL,
  updated_ms INTEGER NOT NULL
);
CREATE TABLE melody_references (
  id INTEGER PRIMARY KEY,
  song_id TEXT NOT NULL REFERENCES songs(id),
  source TEXT NOT NULL CHECK (source IN ('midi','json','manual','extracted')),
  format_version INTEGER NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1,  -- +1 a cada mudança de notas ou offset
  status TEXT NOT NULL CHECK (status IN ('processing','experimental','validated','invalid')),
  offset_ms INTEGER NOT NULL DEFAULT 0,
  notes_json TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 1,    -- no máximo 1 ativa por música
  created_ms INTEGER NOT NULL,
  updated_ms INTEGER NOT NULL
);
CREATE UNIQUE INDEX one_active_reference ON melody_references(song_id) WHERE active = 1;
CREATE TABLE participants (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE, created_ms INTEGER NOT NULL);
CREATE TABLE game_sessions (
  id INTEGER PRIMARY KEY,
  mode TEXT NOT NULL CHECK (mode IN ('solo','party','duel','challenge')),
  name TEXT,
  status TEXT NOT NULL CHECK (status IN ('active','completed','abandoned')),
  config_json TEXT NOT NULL DEFAULT '{}',   -- minRounds, targetScore, difficulty, seed
  started_ms INTEGER NOT NULL,
  completed_ms INTEGER
);
CREATE TABLE rounds (
  id INTEGER PRIMARY KEY,
  session_id INTEGER NOT NULL REFERENCES game_sessions(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  song_id TEXT NOT NULL REFERENCES songs(id),
  participant_id INTEGER REFERENCES participants(id),
  status TEXT NOT NULL CHECK (status IN ('pending','completed','skipped')),
  UNIQUE (session_id, position)
);
CREATE TABLE performances (
  id INTEGER PRIMARY KEY,
  round_id INTEGER REFERENCES rounds(id),
  song_id TEXT NOT NULL REFERENCES songs(id),
  participant_id INTEGER REFERENCES participants(id),
  reference_id INTEGER REFERENCES melody_references(id),
  reference_revision INTEGER,
  algorithm_version INTEGER NOT NULL,
  difficulty TEXT NOT NULL,
  competitive INTEGER NOT NULL,       -- 1 só se referência validated e sem seek
  score REAL NOT NULL,
  metrics_json TEXT NOT NULL,         -- ScoreResult completo
  capture_json TEXT NOT NULL,         -- dispositivo, noise floor, latency, clipping %
  created_ms INTEGER NOT NULL
);
CREATE INDEX perf_leaderboard ON performances(song_id, reference_id, reference_revision, algorithm_version, difficulty, score DESC) WHERE competitive = 1;
