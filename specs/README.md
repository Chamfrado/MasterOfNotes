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
| [SPEC-011](SPEC-011-user-docs.md) | Documentação do usuário | Agente 5 | — |
| [SPEC-012](SPEC-012-auto-reference.md) | Referência automática e alinhamento automático | Agente 2 | — |
| [SPEC-013](SPEC-013-ultrastar.md) | Importar referência UltraStar (.txt) | Agente 2 | — |
| [SPEC-014](SPEC-014-stabilization.md) | Estabilização (segurança, dependências, Ponytail, arquitetura, docs) | Agente 0 | AC-016 |

## Matriz de rastreabilidade (AC → spec → teste → evidência)

Atualizada pelo coordenador em 2026-11-19. ✅ executado com evidência · 🟡 automatizado/sintético verde, falta a validação manual com hardware real · ⏳ pendente.

| AC | Spec | Teste automatizado | Evidência / estado |
|---|---|---|---|
| AC-001 instalar e executar | 001, 009 | build release + smoke | ✅ instalado e executado no Windows 11, sem UAC (`update-test.md`); v0.1.1 instalada pelo proprietário |
| AC-002 selecionar pasta | 002 | `library::tests` (9), E2E smoke | 🟡 `set_library_folder` real no WebView2 (passeio do Agente 1); diálogo nativo → roteiro passo 1 |
| AC-003 listar/pesquisar | 002, 010 | `Library.test.tsx`, E2E busca sem acento | 🟡 roteiro passo 2 |
| AC-004 reproduzir vídeo | 003 | E2E Player + vídeo sintético, `test-song.e2e` | 🟡 vídeo tocou no WebView2 real com CSP de produção; formatos → `codecs.md` |
| AC-005 selecionar/testar mic | 004 | `audio-engine` (sintéticos + hardware ignorado) | ⏳ captura funciona, mas o mic padrão está mudo/bloqueado (−105 dBFS) → roteiro passo 6 |
| AC-006 calibrar | 004 | `calibrate_noise`, `Mic.test.tsx` | ⏳ roteiro passos 7, 10, 11 |
| AC-007 referência válida | 005 | `reference` (Rust), `Reference.test.tsx`, `test_song.rs` | 🟡 roteiro passos 8–9 |
| AC-008 afinado/desafinado/silêncio/atraso | 006 | regressão v2 com golden (pipeline áudio → score) | ✅ sintético (`scoring-accuracy.md`, `scoring-review.md`); ⏳ voz real, roteiro 12–15 |
| AC-009 feedback coerente | 006, 010 | `Player.test.tsx`, regressão | ⏳ roteiro 12–14 |
| AC-010 salvar performance | 007 | `live::tests` (persistência, competitive ≥ 50%) | 🟡 roteiro 16 |
| AC-011 leaderboard persistente | 007 | `game` + `live` + E2E Ranking | 🟡 roteiro 16, 19 |
| AC-012 partida Festa | 008 | `game` (15), telas + E2E Festa (em andamento) | ⏳ roteiro 17–18 |
| AC-013 persistir após reabrir | 001 | `db::reopen_preserves_data`; dados sobreviveram ao update | ✅ via update 0.1.0 → 0.1.1 (`update-test.md`); roteiro 19 |
| AC-014 instalador | 009 | `pnpm build` + workflow Release | ✅ NSIS por usuário; v0.1.1 e v0.1.2 publicadas e assinadas |
| AC-015 update assinado | 009 | assinatura adulterada recusada + update real | ✅ chave de teste (`update-test.md`) **e produção**: 0.1.1 → 0.1.2 pela verificação automática, confirmado pelo proprietário (2026-11-20) |
| AC-016 docs/specs/leis | — | `pnpm check`, CI | 🟡 revisão final do coordenador na Fase 7 |

Evidências em `docs/testing/evidence/`.
