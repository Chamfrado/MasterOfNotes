# SPEC-007 — Performances e leaderboards

## SPEC
- R1. Toda performance finalizada **com referência** é salva em `performances` (competitiva ou treino). Sem referência: nada é salvo como score.
- R2. `competitive = 1` somente se: referência `validated` no início **e** sem seek **e** pelo menos 50% da duração da música foi tocada.
- R3. Leaderboard por música: só `competitive = 1` com a mesma chave de compatibilidade (referência ativa, revisão, `ALGORITHM_VERSION` atual, dificuldade).
- R4. Linha: posição, nome (ou "Anônimo"), score, data. Empate: mais antigo primeiro. Top 50.
- R5. Resultado final mostra recorde pessoal e recorde da música na mesma chave.
- R6. Performances de chaves antigas continuam no histórico, sinalizadas como "versão anterior".

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-007-01 | `game::leaderboard` + `CompatKey` + testes | Agente 4 | done |
| T-007-02 | Command `get_leaderboard` + repositório | Agente 1 | review |
| T-007-03 | Telas de Resultado e Ranking | Agente 3 | todo |

## TEST
`game` testes: `leaderboard_excludes_incompatible`, `ties_older_first`, `top_50`.

## STATUS
- T-007-01 em review (Agente 4, 2026-11-19): `game::leaderboard` + `CompatKey`; testes `leaderboard_excludes_incompatible`, `ties_older_first`, `top_50` ✅.
- T-007-02..03 pendentes.
- T-007-01 done (Agente 0, 2026-11-19): integrado em main, testes verdes.
- T-007-02 `review` (Agente 1, 2026-11-19): `get_leaderboard(songId, difficulty)` em `src-tauri/src/sessions.rs`, usando `game::leaderboard` com a CompatKey atual (referência ativa + revisão + `ALGORITHM_VERSION` + dificuldade). Sem referência → lista vazia. Nome "Anônimo" para participante nulo. `performanceId` é remapeado a partir de (participante, score, data). Testes ✅ `sessions::tests::leaderboard_current_key_only`.
