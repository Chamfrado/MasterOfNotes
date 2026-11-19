# Specs

Um arquivo por spec, com seções **SPEC** (comportamento + critérios), **PLAN**, **TASKS** (`T-NNN-XX`, dono), **TEST** (teste ↔ critério) e **STATUS**. Donos de tarefas atualizam a seção STATUS e a coluna Status das próprias tarefas; o resto é do coordenador.

Status de tarefa: `todo` · `doing` · `review` (entregue, aguardando coordenador) · `done` (integrado em `main`, testes verdes) · `blocked`.

| Spec | Tema | Dono principal | MVP |
|---|---|---|---|
| [SPEC-001](SPEC-001-desktop-app.md) | Aplicação desktop, settings, persistência | Agente 1 | AC-001, AC-013 |
| [SPEC-002](SPEC-002-library.md) | Catálogo local | Agente 1 | AC-002, AC-003 |
| [SPEC-003](SPEC-003-playback.md) | Reprodução de vídeo | Agente 3 | AC-004 |
| [SPEC-004](SPEC-004-microphone.md) | Captura, análise e calibração do microfone | Agente 2 | AC-005, AC-006 |
| [SPEC-005](SPEC-005-melody-reference.md) | Referência melódica | Agente 2 | AC-007 |
| [SPEC-006](SPEC-006-scoring.md) | Pontuação | Agente 2 | AC-008, AC-009 |
| [SPEC-007](SPEC-007-leaderboards.md) | Performances e leaderboards | Agente 4 | AC-010, AC-011 |
| [SPEC-008](SPEC-008-game-modes.md) | Modos Solo, Festa, Duelo, Desafio | Agente 4 | AC-012 |
| [SPEC-009](SPEC-009-updater.md) | Distribuição e atualização | Agente 1 | AC-014, AC-015 |
| [SPEC-010](SPEC-010-design-system.md) | Design system e telas | Agente 3 | — |

## Matriz de rastreabilidade (AC → spec → teste → evidência)

| AC | Spec | Teste | Evidência |
|---|---|---|---|
| AC-001 instalar e executar | 001, 009 | `tauri build` + instalação manual | ⏳ |
| AC-002 selecionar pasta | 002 | `library::tests` + e2e | ⏳ |
| AC-003 listar/pesquisar | 002, 010 | `Library.test.tsx` | ⏳ |
| AC-004 reproduzir vídeo | 003 | checklist manual por formato | ⏳ |
| AC-005 selecionar/testar mic | 004 | manual com dispositivo real | ⏳ |
| AC-006 calibrar | 004 | testes de noise floor + manual | ⏳ |
| AC-007 referência válida | 005 | `scoring-engine` testes de referência | ⏳ |
| AC-008 afinado/desafinado/silêncio/atraso | 006 | `scoring-engine/tests/regression.rs` | ⏳ |
| AC-009 feedback coerente | 006, 010 | regressão + manual | ⏳ |
| AC-010 salvar performance | 007 | `db` testes | ⏳ |
| AC-011 leaderboard persistente | 007 | `game` + `db` testes | ⏳ |
| AC-012 partida Festa | 008 | `game` testes + e2e | ⏳ |
| AC-013 persistir após reabrir | 001 | `db` teste de reabertura + manual | ⏳ |
| AC-014 instalador | 009 | `tauri build` | ⏳ |
| AC-015 update assinado | 009 | release de teste local | ⏳ |
| AC-016 docs/specs/leis | — | revisão do coordenador | ⏳ |

Evidências em `docs/testing/evidence/`.
