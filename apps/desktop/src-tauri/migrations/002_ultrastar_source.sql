-- Migration 002 (SPEC-013 R5): melody_references.source also accepts 'ultrastar'.
-- SQLite cannot alter a CHECK constraint, so the table is rebuilt with the official procedure
-- (https://www.sqlite.org/lang_altertable.html#otheralter): the runner turns foreign_keys OFF
-- outside the transaction and runs PRAGMA foreign_key_check before committing.
-- Ids are kept, so performances.reference_id stays valid. Never edit after integration.
CREATE TABLE melody_references_new (
  id INTEGER PRIMARY KEY,
  song_id TEXT NOT NULL REFERENCES songs(id),
  source TEXT NOT NULL CHECK (source IN ('midi','json','manual','extracted','ultrastar')),
  format_version INTEGER NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1,  -- +1 a cada mudança de notas ou offset
  status TEXT NOT NULL CHECK (status IN ('processing','experimental','validated','invalid')),
  offset_ms INTEGER NOT NULL DEFAULT 0,
  notes_json TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 1,    -- no máximo 1 ativa por música
  created_ms INTEGER NOT NULL,
  updated_ms INTEGER NOT NULL
);
INSERT INTO melody_references_new (id, song_id, source, format_version, revision, status, offset_ms, notes_json, active, created_ms, updated_ms)
  SELECT id, song_id, source, format_version, revision, status, offset_ms, notes_json, active, created_ms, updated_ms
  FROM melody_references;
DROP TABLE melody_references;
ALTER TABLE melody_references_new RENAME TO melody_references;
CREATE UNIQUE INDEX one_active_reference ON melody_references(song_id) WHERE active = 1;
