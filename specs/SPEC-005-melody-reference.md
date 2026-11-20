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

- R8 (ADR-0004 A). **Cantar a referência:** na tela Referência, "Gravar cantando" toca o vídeo enquanto alguém canta a melodia; o app grava só as notas detectadas como nova referência `experimental` (fonte cantada), que depois é revisada e validada como qualquer outra. Com menos de 3 notas, nada é salvo e o app explica. O áudio nunca é gravado.
- R9 (ADR-0004 B, 0.1.3). **Modelo opcional** baixado pelo usuário para gerar a referência de vídeos com voz. Atenção: análise jurídica do proprietário antes da comercialização.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-005-01 | Tipos, parser JSON, import MIDI (`midly`, tempo map) + testes | Agente 2 | done |
| T-005-02 | Fixtures `tests/fixtures/references/` (sintéticas, autoria própria) | Agente 2 | done |
| T-005-03 | Commands de referência + persistência | Agente 1 | done |
| T-005-07 | Cantar a referência: `segment_notes` + modo `reference_capture` no performance.rs (Agente 2); command e persistência (Agente 1); UI "Gravar cantando" na tela Referência (Agente 3) | Agentes 2, 1 e 3 | done |
| T-005-09 | E2E de "Gravar cantando": fake-backend com `referenceCapture` (regras do live.rs), `e2e/reference-capture.e2e.ts`, painel de gravação no axe/teclado | Agente 4 | review |
| T-005-08 | Modelo opcional baixado pelo usuário (ADR-0004 B) | Agentes 2, 1 e 3 | todo (0.1.3) |
| T-005-04 | Tela de referência (import, trilha, offset, validar) | Agente 3 | done |

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
- T-005-03 `review` (Agente 1, 2026-11-19): `src-tauri/src/reference.rs`. `get_reference`, `list_midi_tracks`, `import_reference` (só caminho absoluto, extensões .mid/.midi/.json, até 5 MB; nasce `experimental`; numa transação desativa a ativa anterior, que fica para o histórico), `set_reference_offset` (±10 s; revision+1 e volta a `experimental`; mesmo offset = sem efeito), `set_reference_status` (experimental/validated/invalid). Só a referência ativa pode ser alterada. Erros: `invalid_reference` (mensagem do RefError), `midi_track_required` (MIDI com várias trilhas sem escolha), `invalid_input`, `not_found`. Testes ✅ `reference::tests::{import_json_and_reimport_keeps_history, import_midi_file, rejects_bad_files_and_saves_nothing, offset_bumps_revision_and_resets_status}`.
- T-005-03 done (Agente 0, 2026-11-19): integrado em main; clippy --workspace e cargo test --workspace (92) verdes.
- T-005-04 em review (Agente 3, 2026-11-19): `src/screens/Reference.tsx`, rota `reference`, acessível pelo botão "Referência" de cada música na Biblioteca e por "Importar referência" no Resultado sem referência.
  - Import: diálogo nativo filtrado para .mid/.midi/.json. JSON importa direto. MIDI chama `listMidiTracks` e ignora trilhas sem notas; com uma trilha, importa direto; com várias, abre o diálogo "Qual trilha é a voz?" com nome e nº de notas (R1). Erros do Rust aparecem em toast e nada muda na tela (R2).
  - R3: `<video>` com controles nativos e piano roll em canvas sobre o terço superior (janela −2 s/+4 s, playhead, nota ativa em destaque), desenhado a partir de `video.currentTime`. A geometria fica em `rollLayout`, função pura.
  - R4: slider de ±10 s (passo 10 ms) e botões −100/−10/+10/+100 ms; a prévia usa o offset ainda não salvo. "Salvar offset" chama `setReferenceOffset`, e o selo mostra a revisão e o status que voltam do Rust. "Validar alinhamento" fica bloqueado com offset não salvo. Há também "Marcar inválida".
  - Convenção conferida em `scorer.rs`: offset positivo atrasa as notas (`start + offset`).
  - Testes: `Reference.test.tsx` (9: `rollLayout` ×3, import JSON, escolha de trilha MIDI, MIDI de uma trilha, erro de import, salvar offset → validar, limites ±10 s).
  - ⏳ Canvas e vídeo não vistos no WebView2 real (o jsdom não tem canvas; o desenho é ignorado nos testes).
- T-005-04 done (Agente 0, 2026-11-19): integrado em main @ 52a8b95; typecheck, vitest 158/158, Playwright 43/43 + test-song 1/1, cargo test -p game 16 verdes.
- T-005-06 `review` (Agente 2, 2026-11-19): spike de referência automática a partir do vídeo, em [auto-reference-spike.md](../docs/research/auto-reference-spike.md), com rascunho de [ADR-0004](../docs/adr/ADR-0004-auto-reference.md) para aprovação do Agente 0. Medido: separação (HTDemucs / MDX-Net ONNX) + YIN em voz sintética dá 97–98% de precisão e pontuação simulada de 98,2–98,7; nenhum modelo de separação tem licença comercial clara dos pesos; Basic Pitch (Apache-2.0) sem separação dá 70–78; "cantar a referência" dá 98,6 sem modelo; DTW de alinhamento com mediana de 7 ms. Sem código de produto. ⏳ Precisão com voz real (Vídeo A sem verdade anotada).
- T-005-07, parte do Agente 1 em `review` (2026-11-19). Implementada em `src-tauri/src/live.rs` e `reference.rs`.
  - `StartPerformance.referenceCapture` vira `PerformanceConfig.reference_capture`.
  - O modo roda sozinho: com `leakCheck`, `latencyProbe`, rodada ou participante dá `invalid_input`.
  - A música não precisa de referência. O modo nunca é competitivo e a referência ativa atual não entra na sessão.
  - Ao terminar, com ≥ 3 `captured_notes`, as notas passam por `reference::store_captured` (`to_reference_json` → `parse_reference_json`, o mesmo parser do import). Viram a nova referência ativa (`manual`, `experimental`, revision 1, offset 0); a anterior é desativada e fica no histórico. A referência volta em `outcome.capturedReference`.
  - Com menos de 3 notas, nada é gravado e vem `reason` "Poucas notas detectadas: cante a melodia mais perto do microfone e tente de novo.".
  - Nunca grava nada em `performances`. O `import` e a captura agora usam a mesma função de inserção (`reference::store`).
  - Testes ✅ `live::tests::reference_capture_stores_notes_as_new_active_reference`. `cargo test -p master-of-notes`: 61 ok, 7 ignorados (com a branch do Agente 2 @ 6612c44 mesclada).
- T-005-07, parte UI, `review` (Agente 3, 2026-11-19): botão "Gravar cantando" na tela Referência, também visível sem referência. Quando a referência ativa é cantada (`source = manual`), o botão vira "Gravar de novo".
  - O painel explica: "Cante a melodia junto com o vídeo; o app grava só as notas, nunca a sua voz. Use fones de ouvido." Se já existe referência, avisa que a atual fica no histórico.
  - Fluxo: `startPerformance({referenceCapture: true})` → contagem 3-2-1 → o vídeo toca do início. A sincronia usa `startVideoSync` (relógio de mídia) e play/pause, como no Player. Durante a gravação aparecem o medidor do microfone e o selo "Gravando notas". O selo fica no cabeçalho do painel, fora do vídeo, num `role="status"` (correção 0.1.2: antes cobria o canto do vídeo).
  - "Parar" (ou o fim do vídeo) pausa, envia o último sync e chama `finishPerformance`. Se `capturedReference` vier, a tela volta ao editor com as notas no piano roll, selo "Cantada" + "Referência experimental", e os botões de sempre: offset, Validar, Gravar de novo. Se vier `null`, mostra o `reason` do Rust e oferece "Gravar de novo". "Cancelar" e sair da tela chamam `abortPerformance`.
  - Testes: `Reference.test.tsx`, +3 (gravação sem referência → experimental; `null` com reason ao fim do vídeo; cancelar aborta e mantém a referência validada).
  - ⏳ Não visto no WebView2 real com microfone; a fake-backend do e2e não simula `referenceCapture`.
- T-005-07, parte do Agente 2 `review` (2026-11-19): `scoring_engine::segment_notes` (Viterbi sobre semitons, custo de troca 8, trechos < 80 ms absorvidos, correção de oitava, semitom A440 escolhido por evidência contra fracionário e grade com afinação) e o modo `reference_capture` no `performance.rs` (`PerformanceSummary.captured_notes`; seek regrava a partir do ponto). Voz realista com ±40 ms de erro de tempo: recall 92–100%, precisão 82–100%, cantor perfeito contra a referência gravada 95,0–98,8. Números em [scoring-accuracy.md](../docs/testing/scoring-accuracy.md#gravação-de-referência-cantada-segment_notes-t-005-07). ⏳ Voz real.
- T-005-07 done (Agente 0, 2026-11-19): referência cantada de ponta a ponta (segment_notes + reference_capture do Agente 2, persistência do Agente 1, UI do Agente 3). Validação em main abaixo. ⏳ gravação real com microfone e voz.
- T-005-09 `review` (Agente 4, 2026-11-19): E2E da referência cantada contra o fake com estado.
  - `e2e/fake-backend.ts` passa a ter referências: `get_reference` mostra só a ativa e `set_reference_status` muda o status.
  - Na captura, `start_performance({referenceCapture: true})` segue as regras do `live.rs`: roda sozinha (com teste de vazamento, sonda, rodada ou participante dá `invalid_input`) e nunca é competitiva.
  - `finish_performance` com ≥ 3 notas grava uma referência `manual`, `experimental`, revisão 1, offset 0, que vira a ativa; a anterior fica no histórico, inativa. Com < 3 notas devolve `capturedReference: null` e o motivo "Poucas notas detectadas…".
  - As notas "detectadas" vêm de `opts.captures`.
  - `e2e/reference-capture.e2e.ts` (4 casos):
    1. Música sem referência → contagem 3-2-1 → "Gravando notas" → Parar → selos Cantada, experimental, Revisão 1 e 5 notas → Validar alinhamento.
    2. Poucas notas: aparece o motivo, nada é salvo, e "Gravar de novo" funciona.
    3. Cancelar durante a gravação chama `abort_performance` e não chama `finish_performance`.
    4. Gravar sobre uma referência existente mostra o aviso de histórico; a cantada vira a ativa e continua depois de reabrir o app.
  - `a11y.e2e.ts`: telas "Gravar cantando" (apresentação, com aviso de substituição) e "Gravando notas" na rodada de axe, teclado, viewports e movimento reduzido.
  - ❓ O fake não detecta notas; a segmentação real é testada no Rust (Agente 2).
