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
| T-005-01 | Tipos, parser JSON, import MIDI (`midly`, tempo map) + testes | Agente 2 | todo |
| T-005-02 | Fixtures `tests/fixtures/references/` (sintéticas, autoria própria) | Agente 2 | todo |
| T-005-03 | Commands de referência + persistência | Agente 1 | todo |
| T-005-04 | Tela de referência (import, trilha, offset, validar) | Agente 3 | todo |

## TEST
`scoring-engine` testes de referência: JSON válido/inválido, MIDI com tempo map, sobreposição, trilhas.

## STATUS
Pendente.
