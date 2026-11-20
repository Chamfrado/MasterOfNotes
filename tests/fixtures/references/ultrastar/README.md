# UltraStar fixtures (SPEC-013, T-013-04)

Self-authored for Master Of Notes tests (Law 19): invented melodies and syllables, no third-party song.
Written by the Agent 4 QA script, archived in `docs/testing/evidence/ultrastar-qa.md`. The beat is
`60000 / (BPM × 4)` ms and the pitch is `midi = 60 + pitch` (SPEC-013 R2).

## Valid: expected notes `(start_ms, dur_ms, midi, lyric)`
| File | What it tests | Expected |
|---|---|---|
| `basic.txt` | Criterion of the SPEC: BPM 300, GAP 1000; golden `*`; `F`/`R`/`G` skipped; `-` break; `~` continuation | (1000, 200, 60, "Lá"), (1200, 200, 62, "ri"), (1900, 200, 64, "la"), (2100, 100, 65, "~") |
| `bpm-comma.txt` | `#BPM:150,5` (decimal comma, common in real files) | beat 99.67 ms: (0, ≈399, 60), (≈797, ≈399, 62) (±1 ms rounding) |
| `relative.txt` | `#RELATIVE:yes`, GAP 500; `- 10 12` moves the base 12 beats | (500, 200, 60), (700, 200, 62), (1100, 200, 64), (1400, 100, 65) |
| `duet.txt` | `P1`/`P2`: only P1, plus the duet notice | (0, 200, 60, "eu"), (200, 200, 62, "vou"); `duet = true` |
| `after-end.txt` | Lines after `E` are ignored | (0, 200, 60, "fim") only |
| `cp1252.txt` | Latin-1/CP1252 without `#ENCODING` | lyrics "Ação", "pé"; title "Canção de Teste" |
| `bom.txt` | UTF-8 with BOM before `#TITLE` | title "Canção de Teste"; (0, 200, 60, "oi") |
| `crlf.txt` | CRLF line endings | (0, 200, 60, "oi"), (200, 200, 62, "tchau"), no `\r` in lyrics |
| `duet-p-space.txt` | `P 1` / `P 2` (with a space, seen in real files) | (0, 200, 60, "eu") only; `duet = true` |

## Invalid: expected a pt-BR error, with the line number when one line is at fault (R4); never a panic
| File | Problem | Expected |
|---|---|---|
| `invalid-bpm-zero.txt`, `invalid-bpm-negative.txt`, `invalid-bpm-text.txt` | BPM 0 / −120 / not a number | error about BPM |
| `invalid-bpm-missing.txt` | no `#BPM` (mandatory) | error about BPM |
| `invalid-truncated-line.txt` | `: 4 4` (no pitch) | error at line 4 |
| `invalid-pitch-high.txt` | pitch 49 → MIDI 109 (> 108) | error at line 4 |
| `invalid-pitch-low.txt` | pitch −37 → MIDI 23 (< 24) | error at line 3 |
| `invalid-negative-length.txt` | length −4 | error at line 3 |
| `invalid-only-freestyle.txt` | only `F`/`R`/`G` | error: no scorable note |
| `invalid-empty.txt` | empty file | error |
| `invalid-huge-numbers.txt` | beat 2³², length 10²⁰ (integer overflow) | error at line 3, no panic |

## Edge: either behaviour is acceptable if documented, never a panic or a wrong silent result
| File | Problem | Acceptable |
|---|---|---|
| `edge-gap-negative.txt` | `#GAP:-500`: the first note would start at −500 ms | error at line 3, or the note is dropped/clamped (documented) |
| `edge-overlap.txt` | `: 0 8 0` then `: 4 4 2` (overlap) | the same normalisation as JSON/MIDI (trim), or error at line 4 |
| `edge-header-case.txt` | `#bpm: 300`, `#Gap : 0` (case and spaces) | accepted (BPM 300), or a clear BPM error |
| `edge-no-syllable.txt` | `: 0 4 0` without a syllable | note with an empty/absent lyric, or error at line 3 |
