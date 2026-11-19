# SPEC-005 — Referência melódica

Decisão: [ADR-0002](../docs/adr/ADR-0002-melody-reference.md).

## SPEC
- R1. Importar `.mid/.midi` (usuário escolhe a trilha; lista com nome e nº de notas) e `.json` (`mon-melody` v1).
- R2. Validação: ≥ 1 nota; durações > 0; MIDI 24–108; ordenação por início; sobreposições resolvidas mantendo a nota mais aguda. Falha → erro claro, nada salvo.
- R3. Referência importada nasce `experimental`. O usuário marca `validated` na tela de referência após conferir o alinhamento (notas desenhadas sobre o vídeo).
- R4. Ajuste de offset (±10 s, passo 10 ms) incrementa `revision` e volta o status para `experimental`.
- R5. Uma referência ativa por música; reimportar desativa a anterior (mantida para o histórico).
- R6. Estados para a UI: `none`, `processing`, `experimental`, `validated`, `invalid`.
- R7. MIDI: respeitar tempo map (mudanças de BPM) e ticks-per-beat; SMPTE timecode → erro "formato não suportado".

Critérios:
- Given MIDI com trilha de piano e trilha "Vocal", When listo trilhas, Then ambas aparecem com nº de notas.
- Given MIDI com mudança de BPM no meio, Then os tempos das notas após a mudança estão corretos (±1 ms).
- Given referência validada, When mudo o offset, Then status = experimental e revision + 1.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-005-01 | Tipos, parser JSON, import MIDI (`midly`, tempo map) + testes | Agente 2 | done |
| T-005-02 | Fixtures `tests/fixtures/references/` (sintéticas, autoria própria) | Agente 2 | done |
| T-005-03 | Commands de referência + persistência | Agente 1 | todo |
| T-005-04 | Tela de referência (import, trilha, offset, validar) | Agente 3 | todo |

## TEST
`scoring-engine` testes de referência: JSON válido/inválido, MIDI com tempo map, sobreposição, trilhas.

## STATUS
- T-005-01/02 `review` (Agente 2, 2026-11-19): `parse_reference_json`, `to_reference_json`, `import_midi`, `midi_tracks` e validação. Fixtures de autoria própria em `tests/fixtures/references/`. 9 testes verdes, incluindo os critérios de trilhas e de mudança de BPM (tempos exatos ao ms).
- Sobreposição ("manter a mais aguda"), implementada como skyline:
  - acorde com o mesmo início: fica a nota mais aguda;
  - nota mais aguda que começa sobre outra: corta a de baixo;
  - nota mais grave que começa sob uma mais aguda: fica só a cauda dela depois do fim da aguda (legato descendente desloca poucos ms), ou é descartada se estiver totalmente coberta.
- MIDI: `track = None` escolhe a única trilha com notas; com várias, dá o erro `TrackRequired`. MIDI tipo 2 e SMPTE: erro. Notas que arredondam para 0 ms são descartadas. Letras de MIDI não são importadas (`lyric = None`).
- Critério "offset → experimental e revision+1" é persistência (T-005-03, Agente 1).
- Tarefas review → done (Agente 0, 2026-11-19): integrado em main @ 566a64e; fmt, clippy --workspace -D warnings e cargo test --workspace verdes (80 testes).
