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
| T-008-01 | `crates/game`: `plan_rounds`, `party_standings`, `duel_outcome`, `challenge_progress` + testes | Agente 4 | review |
| T-008-02 | Commands de participantes/sessões + repositórios | Agente 1 | todo |
| T-008-03 | Telas: modo, participantes, festa, confronto, histórico | Agente 3 | todo |
| T-008-04 | E2E Playwright do fluxo Festa (IPC mockado apenas no teste) | Agente 4 | todo |

## TEST
`game` testes: média vs total, elegibilidade, desempate, shuffle determinístico, duelo incompatível, desafio.

## STATUS
- T-008-01 em review (Agente 4, 2026-11-19): `crates/game` implementado; `cargo test -p game` 15/15 ✅ (média vs total, elegibilidade, desempate total→id, anônimos ignorados, shuffle determinístico, cobertura do plano, duelo incompatível, empate por arredondamento a 0,1, desafio).
- T-008-02..04 pendentes.
