# SPEC-008 — Modos de jogo

Contrato: [contracts.md §2.3](../docs/architecture/contracts.md#23-game-agente-4--puro-sem-io).

## SPEC
- **Solo**: 1 música, nome opcional; sessão `solo` com 1 rodada; resultado com métricas e comparação com recordes; "Repetir".
- **Festa**: nome; ≥ 2 participantes; ≥ 1 música; ordem manual ou sorteada (seed salva → reproduzível); rodadas (participante, música); ranking parcial após cada rodada com **total e média**; classificação por **média** entre elegíveis (`rounds ≥ minRounds`; padrão = menor nº de rodadas atribuídas a um participante); desempate por total e depois ordem de cadastro; vencedor ao final; festa `active` pode ser retomada após fechar o app.
- **Duelo**: 2 participantes, mesma música, apresentações separadas, tela de confronto; comparação só com a mesma `CompatKey`, senão erro explícito.
- **Desafio**: fila de músicas, nº de rodadas, meta de pontuação média; progresso, sucesso/falha.
- Rodada só fica `completed` com performance competitiva. Festa/Duelo/Desafio só aceitam músicas com referência `validated` (UI explica por quê).

Critérios:
- Given A com 3 rodadas (média 80, total 240) e B com 2 rodadas (média 90, total 180) e minRounds 2, Then B em 1º.
- Given minRounds 3, Then B inelegível e aparece abaixo dos elegíveis.
- Given mesmo seed e entradas, Then `plan_rounds` produz a mesma ordem.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-008-01 | `crates/game`: `plan_rounds`, `party_standings`, `duel_outcome`, `challenge_progress` + testes | Agente 4 | done |
| T-008-02 | Commands de participantes/sessões + repositórios | Agente 1 | review |
| T-008-03 | Telas: modo, participantes, festa, confronto, histórico | Agente 3 | todo |
| T-008-04 | E2E Playwright do fluxo Festa (IPC mockado apenas no teste) | Agente 4 | doing |

## TEST
`game` testes: média vs total, elegibilidade, desempate, shuffle determinístico, duelo incompatível, desafio.

## STATUS
- T-008-01 em review (Agente 4, 2026-11-19): `crates/game` implementado; `cargo test -p game` 15/15 ✅ (média vs total, elegibilidade, desempate total→id, anônimos ignorados, shuffle determinístico, cobertura do plano, duelo incompatível, empate por arredondamento a 0,1, desafio).
- T-008-04 em andamento (Agente 4, 2026-11-19): infraestrutura E2E pronta: `apps/desktop/playwright.config.ts` (Vite na porta 1440), `e2e/tauri-mock.ts` (mock de `__TAURI_INTERNALS__` só em teste, respostas por comando, `setReply`, registro de chamadas), `e2e/smoke.e2e.ts` com 6 fluxos verdes, E2E no CI. Fluxo Festa ⏳ aguarda as telas do Agente 3.
- T-008-01 done (Agente 0, 2026-11-19): integrado em main, `cargo test -p game` 15/15 ✅ e clippy limpo após merge. Nota para T-008-02: `party_standings` só lista quem tem performance; o glue deve acrescentar participantes da festa com 0 rodadas (inelegíveis) para exibição. `challenge_progress` exige `rounds ≥ 1` (validar no command).
- E2E smoke (Agente 0, 2026-11-19): 6/6 Playwright verdes em main @ a04004d. T-008-04 segue em doing (fluxo Festa).
- T-008-02 `review` (Agente 1, 2026-11-19): `src-tauri/src/sessions.rs`.
  - Comandos: `list_participants`, `create_participant` (1–40 caracteres, único sem distinção de caixa → `participant_exists`), `create_session`, `get_session`, `list_sessions`, `set_round_status` (só pending/skipped, rodada não concluída, sessão ativa), `complete_session`.
  - Regras de `create_session`:
    - solo: 1 música e 0–1 participante; aceita música sem referência;
    - festa: ≥ 2 participantes e ≥ 1 música;
    - duelo: 2 participantes e 1 música;
    - desafio: ≥ 1 música, 0–1 participante, meta 0–100 obrigatória e uma rodada por música;
    - festa/duelo/desafio exigem referência `validated` (`reference_not_validated`);
    - participantes repetidos ou inexistentes são recusados.
  - Rodadas: `game::plan_rounds` com seed < 2^53 salva em `config`. `minRounds` padrão = menor nº de rodadas de um participante.
  - Standings: `game::party_standings` sobre as rodadas concluídas, mais os participantes com 0 rodadas no fim, inelegíveis. Rodada `completed` só via performance competitiva (T-006-05).
  - Testes ✅ `sessions::tests::{participants_unique_case_insensitive, mode_rules, party_standings_average_and_zero_round_participants, shuffle_is_reproducible_from_saved_seed}`.
