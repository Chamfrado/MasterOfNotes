"""Independent QA of migration 002 (SPEC-013 R5, T-013-04). Agent 4. stdlib only.

Usage: python -I migration_qa.py <001_init.sql> <002_ultrastar_source.sql> <work_dir>

Builds a 0.1.3 database (schema 001, user_version 1) filled with data in every table, then applies 002 the
way src-tauri/src/db.rs does (VACUUM INTO backup, foreign_keys OFF, one transaction, foreign_key_check,
user_version) and checks: nothing lost or changed, same schema except the CHECK, index kept, backup equal
to the original, 'ultrastar' accepted, a bad source refused, foreign keys still enforced. Also replays the
"dangling reference" case: the migration must roll back and leave the data at v1.
"""
import os
import re
import shutil
import sqlite3
import sys

sys.stdout.reconfigure(encoding="utf-8")

sql001, sql002, work = sys.argv[1], sys.argv[2], sys.argv[3]
os.makedirs(work, exist_ok=True)
problems = []


def check(cond, msg):
    print(("  ok  " if cond else "  FALHA ") + msg)
    if not cond:
        problems.append(msg)


def dump(conn):
    tables = [r[0] for r in conn.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]
    return {t: sorted(conn.execute(f"SELECT * FROM {t}").fetchall(), key=repr) for t in tables}


def schema(conn):
    return {
        (r[0], r[1]): re.sub(r"\s+", " ", r[2] or "").strip()
        for r in conn.execute("SELECT type, name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'")
    }


def v013(path, dangling=False):
    """A database as 0.1.3 leaves it: schema 001 + data in every table."""
    if os.path.exists(path):
        os.remove(path)
    c = sqlite3.connect(path, isolation_level=None)
    c.execute("PRAGMA journal_mode=WAL")
    c.executescript(open(sql001, encoding="utf-8").read())
    c.execute("PRAGMA user_version=1")
    cols = {t: [r[1] for r in c.execute(f"PRAGMA table_info({t})")] for t in ["songs", "melody_references", "performances", "participants", "game_sessions", "rounds", "settings"]}
    print("colunas 0.1.3:", {t: len(v) for t, v in cols.items()})
    c.execute("PRAGMA foreign_keys=ON")
    return c, cols


def insert(c, table, row):
    keys = ", ".join(row)
    marks = ", ".join("?" for _ in row)
    c.execute(f"INSERT INTO {table} ({keys}) VALUES ({marks})", list(row.values()))


def fill(c, cols, dangling=False):
    """Data in every table, as 0.1.3 writes it (schema 001): history of references, links everywhere."""
    for i, (sid, title, artist) in enumerate([("a1", "Canção da Manhã ç ã é 🎤", "Banda Aurora"), ("b2", "Sem Referência", None), ("c3", "Pôr do Sol", None)]):
        insert(c, "songs", dict(id=sid, path=f"C:/Karaoke/{sid}.mp4", file_name=f"{sid}.mp4", title=title, artist=artist, extension="mp4",
                                size_bytes=1_000_000 + i, modified_ms=1_700_000_000_000 + i, duration_ms=185_000 if i != 1 else None,
                                playable=["yes", "unknown", "no"][i], available=0 if i == 1 else 1, added_ms=1_700_000_000_000, updated_ms=1_700_000_000_500))
    notes = '{"formatVersion":1,"notes":[{"startMs":0,"durMs":500,"midi":60.5,"lyric":"Lá ção"}]}'
    # History on a1 (3 inactive + 1 active, every 0.1.3 source); c3 validated with a big offset.
    for rid, sid, src, st, active, rev, off in [(1, "a1", "midi", "validated", 0, 3, 120), (2, "a1", "json", "experimental", 0, 1, -40),
                                                 (3, "a1", "manual", "invalid", 0, 2, 0), (4, "a1", "extracted", "experimental", 1, 1, 0),
                                                 (5, "c3", "json", "validated", 1, 7, 9999)]:
        insert(c, "melody_references", dict(id=rid, song_id=sid, source=src, format_version=1, revision=rev, status=st, offset_ms=off,
                                            notes_json=notes, active=active, created_ms=1_700_000_000_000 + rid, updated_ms=1_700_000_000_100 + rid))
    for pid, name in [(1, "Ana"), (2, "Bruno Ñ")]:
        insert(c, "participants", dict(id=pid, name=name, created_ms=1_700_000_000_000))
    insert(c, "game_sessions", dict(id=10, mode="party", name="Festa de Sábado", status="completed", config_json='{"minRounds":1,"difficulty":"normal","seed":7}', started_ms=1, completed_ms=2))
    insert(c, "game_sessions", dict(id=11, mode="duel", name=None, status="active", started_ms=3, completed_ms=None))
    for rid, sess, pos, sid, pid, st in [(100, 10, 1, "c3", 1, "completed"), (101, 10, 2, "c3", 2, "skipped"), (102, 11, 1, "a1", 1, "pending")]:
        insert(c, "rounds", dict(id=rid, session_id=sess, position=pos, song_id=sid, participant_id=pid, status=st))
    for k, (rnd, sid, pid, ref, rev, comp) in enumerate([(100, "c3", 1, 5, 7, 1), (None, "a1", 2, 1, 3, 0), (None, "a1", None, 4, 1, 0), (None, "b2", None, None, None, 0)], start=1):
        insert(c, "performances", dict(id=k, round_id=rnd, song_id=sid, participant_id=pid, reference_id=ref, reference_revision=rev, algorithm_version=5,
                                       difficulty="normal", competitive=comp, score=80.5 + k, metrics_json='{"pitch":0.9}', capture_json='{"missingRatio":0.01}',
                                       created_ms=1_700_000_100_000 + k))
    insert(c, "settings", dict(key="app", value='{"libraryFolder":"C:/Karaoke","tvMode":true,"separationModel":{"id":"534b","name":"UVR","acceptedMs":1}}'))
    if dangling:
        c.execute("PRAGMA foreign_keys=OFF")
        insert(c, "performances", dict(id=99, round_id=None, song_id="a1", participant_id=None, reference_id=12345, reference_revision=1, algorithm_version=5,
                                       difficulty="normal", competitive=0, score=0.0, metrics_json="{}", capture_json="{}", created_ms=1))
        c.execute("PRAGMA foreign_keys=ON")
    print("  linhas:", {t: c.execute(f"SELECT count(*) FROM {t}").fetchone()[0] for t in cols})


def migrate(path):
    """Mirror of db.rs migrate() for v1 → v2."""
    c = sqlite3.connect(path, isolation_level=None)
    c.execute("PRAGMA journal_mode=WAL")
    c.execute("PRAGMA foreign_keys=OFF")
    backup = path + ".bak-v1"
    if os.path.exists(backup):
        os.remove(backup)
    c.execute("VACUUM INTO ?", (backup,))
    try:
        c.execute("BEGIN")
        for stmt in [s for s in sqlite3_split(open(sql002, encoding="utf-8").read())]:
            c.execute(stmt)
        violations = c.execute("SELECT count(*) FROM pragma_foreign_key_check").fetchone()[0]
        if violations:
            c.execute("ROLLBACK")
            return c, f"rollback ({violations} violações)"
        c.execute("PRAGMA user_version=2")
        c.execute("COMMIT")
    except sqlite3.Error as e:
        c.execute("ROLLBACK")
        return c, f"erro: {e}"
    finally:
        c.execute("PRAGMA foreign_keys=ON")
    return c, "ok"


def sqlite3_split(script):
    out, buf = [], ""
    for line in script.splitlines(keepends=True):
        if line.strip().startswith("--"):
            continue
        buf += line
        if sqlite3.complete_statement(buf):
            out.append(buf.strip())
            buf = ""
    return out


print("\n## 1. Banco 0.1.3 com dados → migração 002")
db = os.path.join(work, "masterofnotes.db")
c, cols = v013(db)
fill(c, cols)
before, schema_before = dump(c), schema(c)
c.close()
original_copy = os.path.join(work, "original.db")
shutil.copy(db, original_copy)

c, status = migrate(db)
check(status == "ok", f"migração: {status}")
check(c.execute("PRAGMA user_version").fetchone()[0] == 2, "user_version = 2")
after, schema_after = dump(c), schema(c)
for t in before:
    check(before[t] == after.get(t), f"tabela {t}: {len(before[t])} linhas idênticas")
check(set(after) == set(before), "mesmo conjunto de tabelas")
changed = {k for k in set(schema_before) | set(schema_after) if schema_before.get(k) != schema_after.get(k)}
check(changed == {("table", "melody_references")}, f"só a definição de melody_references mudou: {sorted(changed)}")
old, new = schema_before[("table", "melody_references")], schema_after[("table", "melody_references")]
norm = lambda s: s.replace('"melody_references"', "melody_references")  # RENAME quotes the name
check(norm(new).replace(",'ultrastar'", "") == norm(old), "colunas, tipos, defaults e CHECK de status iguais; só 'ultrastar' a mais no CHECK de source")
b0 = sqlite3.connect(original_copy)
struct = lambda conn: (conn.execute("PRAGMA table_info(melody_references)").fetchall(), conn.execute("PRAGMA foreign_key_list(melody_references)").fetchall(),
                       conn.execute("PRAGMA foreign_key_list(performances)").fetchall())
check(struct(b0) == struct(c), "estrutura igual: table_info, FKs de melody_references e a FK de performances → melody_references")
b0.close()
check(("index", "one_active_reference") in schema_after and schema_after[("index", "one_active_reference")] == schema_before[("index", "one_active_reference")], "índice one_active_reference recriado igual")
check(c.execute("SELECT count(*) FROM pragma_foreign_key_check").fetchone()[0] == 0, "foreign_key_check vazio")
check(c.execute("PRAGMA integrity_check").fetchone()[0] == "ok", "integrity_check ok")
perf_refs = c.execute("SELECT p.id, r.source FROM performances p JOIN melody_references r ON r.id = p.reference_id ORDER BY p.id").fetchall()
check(perf_refs == [(1, "json"), (2, "midi"), (3, "extracted")], f"performances seguem ligadas às mesmas referências: {perf_refs}")

c.execute("PRAGMA foreign_keys=ON")
insert(c, "melody_references", {"song_id": "b2", "source": "ultrastar", "format_version": 1, "status": "experimental", "notes_json": "{}", "active": 1, "created_ms": 1, "updated_ms": 1})
check(True, "INSERT com source 'ultrastar' aceito")
for bad, why in [({"source": "karaoke"}, "source fora da lista"), ({"song_id": "nao-existe"}, "song_id inexistente (FK)")]:
    try:
        row = {"song_id": "b2", "source": "json", "format_version": 1, "status": "experimental", "notes_json": "{}", "active": 0, "created_ms": 1, "updated_ms": 1}
        row.update(bad)
        insert(c, "melody_references", row)
        check(False, f"{why} deveria ser recusado")
    except sqlite3.IntegrityError:
        check(True, f"{why} recusado")
try:
    insert(c, "melody_references", {"song_id": "a1", "source": "json", "format_version": 1, "status": "experimental", "notes_json": "{}", "active": 1, "created_ms": 1, "updated_ms": 1})
    check(False, "2ª referência ativa da mesma música deveria ser recusada (one_active_reference)")
except sqlite3.IntegrityError:
    check(True, "2ª referência ativa da mesma música recusada (índice único parcial vale)")
c.close()

backup = db + ".bak-v1"
check(os.path.exists(backup), "backup .bak-v1 criado")
b = sqlite3.connect(backup)
check(b.execute("PRAGMA user_version").fetchone()[0] == 1 and dump(b) == before, "backup = banco 0.1.3 original (v1, mesmos dados)")
b.close()

print("\n## 2. Reabrir (idempotência): já em v2, nada a fazer")
c = sqlite3.connect(db)
check(c.execute("PRAGMA user_version").fetchone()[0] == 2, "continua v2 (o runner sai cedo quando current == target)")
c.close()

print("\n## 3. Referência pendurada (performance → referência inexistente): a migração desfaz tudo")
db2 = os.path.join(work, "dangling.db")
c, cols = v013(db2)
fill(c, cols, dangling=True)
before2 = dump(c)
c.close()
c, status = migrate(db2)
check(status.startswith("rollback"), f"migração recusada: {status}")
check(c.execute("PRAGMA user_version").fetchone()[0] == 1 and dump(c) == before2, "banco continua v1, com os dados intactos")
check("ultrastar" not in (schema(c)[("table", "melody_references")]), "esquema antigo intacto (sem tabela _new nem CHECK novo)")
check(not any(n == "melody_references_new" for _, n in schema(c)), "nenhuma tabela melody_references_new sobrando")
c.close()

print("\n## Resultado:", "SEM FALHAS" if not problems else f"{len(problems)} FALHA(S)")
for p in problems:
    print(" -", p)
