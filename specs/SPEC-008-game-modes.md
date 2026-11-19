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
| T-008-02 | Commands de participantes/sessões + repositórios | Agente 1 | done |
| T-008-03 | Telas: modo, participantes, festa, confronto, histórico | Agente 3 | done |
| T-008-05 | Duelo: `GameSession.duel` via `game::duel_outcome` (CompatKey); UI usa o resultado; E2E resultado + incompatível | Agente 1 + 3 + 4 | todo |
| T-008-04 | E2E Playwright do fluxo Festa (IPC mockado apenas no teste) | Agente 4 | done |

## TEST
`game` testes: média vs total, elegibilidade, desempate, shuffle determinístico, duelo incompatível, desafio.

## STATUS
- T-008-01 em review (Agente 4, 2026-11-19): `crates/game` implementado; `cargo test -p game` 15/15 ✅ (média vs total, elegibilidade, desempate total→id, anônimos ignorados, shuffle determinístico, cobertura do plano, duelo incompatível, empate por arredondamento a 0,1, desafio).
- T-008-04 em review (Agente 4, 2026-11-19): `apps/desktop/e2e/party.e2e.ts` com um backend falso **com estado, só de teste** (`e2e/fake-backend.ts`, que persiste em `localStorage` para simular fechar e reabrir o app). O teste percorre: cadastro de 2 cantores (nome duplicado recusado com a mensagem do backend) → Nova Festa (as 2 músicas sem referência validada não são oferecidas, com a explicação; erros de formulário) → ordem manual, 3 músicas → canto pelo Player com vídeo sintético → ranking parcial com **média, total e rodadas** (Bruno 90/90/1 à frente de Ana 75/150/2: a média vence o total) → recarregar a página → Jogar → "Continuar de onde parou" (faltam 2 rodadas) → Retomar → vencedor ao Encerrar. ✅ 1/1, cerca de 20 s.
- ❌ **Lacuna (não testável):** Duelo com CompatKey incompatível. Ninguém chama `game::duel_outcome`: `sessions.rs` não usa, e o `DuelBoard` da UI (`duelResult`) compara só as notas das rodadas. `GameSession`/`Round` também não têm campo para o erro. Exemplo concreto: se a referência ganha uma revisão entre as duas apresentações, as notas são comparadas sem erro, o que contradiz a SPEC-008. Precisa de decisão de contrato (Agente 0) e implementação (Agentes 1 e 3); o E2E entra depois.
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
- T-008-02 done (Agente 0, 2026-11-19): integrado em main @ 1d88d64; fmt, clippy --workspace -D warnings e cargo test --workspace verdes (112 testes, app 40 + 1 ignorado de hardware).
- T-008-03 em review (Agente 3, 2026-11-19). Rotas `modes` (item "Jogar" na NavBar), `new-session`, `session`, `participants`, `history`.
  - Jogar (`Modes.tsx`): cards Solo/Festa/Duelo/Desafio; "Continuar de onde parou" lista as sessões `active` com o nº de rodadas pendentes (retomar depois de fechar o app); atalhos Participantes e Histórico.
  - Nova partida (`NewSession.tsx`): `validateSession` (pura) espelha `create_session`: festa com nome, ≥ 2 cantores e ≥ 1 música; duelo com 2 e 1; solo com 0–1 e 1; desafio com 0–1, ≥ 1 música e meta 0–100; `minRounds` inteiro ≥ 1 ou vazio (padrão do Rust). Fora do Solo, só aparecem músicas com referência `validated`, e um texto diz quantas ficaram de fora e por quê. Ordem sorteada ou manual (ordem de clique, numerada). Cadastro de cantor no próprio formulário; erros do Rust (nome repetido) aparecem sob o campo.
  - Partida (`Session.tsx`): cartão "Rodada N de M: X canta Y" com Cantar (Player com `roundId`/`participantId`) e Pular; lista de rodadas com status, nota e "Voltar para a fila"; Encerrar pede confirmação se há rodadas pendentes.
  - Festa: `StandingsTable` com média e total, elegíveis primeiro e inelegíveis com "falta N rodada(s)"; ranking parcial e, no fim, o vencedor (rank 1 elegível).
  - Duelo: tela de confronto com o vencedor ou empate. Desafio: meta × média, progresso e cumprido/não cumprido.
  - Player → Resultado usa `replace` (Voltar no Resultado não reabre o Player); numa rodada, "Voltar para a partida" recarrega a sessão.
  - Histórico (`History.tsx`): sessões da mais recente para a mais antiga, com modo, data, status e vencedor.
  - Testes: `Session.test` (8: `currentRound`, `duelResult`, `challengeStatus`, cantar com contexto, ranking parcial, pular, encerrar com confirmação e vencedor, erro), `NewSession.test` (5), `Modes.test` (1), `StandingsTable` (2, incluindo o critério A×B da spec).
  - ⏳ Sem verificação com os commands reais nem no WebView2. O erro de CompatKey do duelo vem do Rust; a tela de confronto compara só as notas das duas rodadas.
- T-008-03 done (Agente 0, 2026-11-19): integrado em main @ 52a8b95; typecheck, vitest 158/158, Playwright 43/43 + test-song 1/1, cargo test -p game 16 verdes.
- T-008-04 done (Agente 0, 2026-11-19): E2E da Festa integrado; suíte completa 95/95 em main. Lacuna encontrada pelo Agente 4: o Duelo não verificava CompatKey → T-008-05.
