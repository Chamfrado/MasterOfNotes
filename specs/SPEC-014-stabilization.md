# SPEC-014 — Estabilização (Fase 7)

## SPEC
- R1. **Segurança:** revisão da superfície IPC e do empacotamento. Cobre: validação de entradas de todos os commands (caminhos, tamanhos, enums), escopo do asset protocol (só os vídeos indexados), upload raw, instalação de modelo (path traversal, links simbólicos, arquivos enormes), CSP, capabilities, updater (assinatura, endpoint), logs (não vazam caminhos fora da biblioteca nem dados pessoais) e o script de download da DLL (SHA fixado). Cada achado com severidade e dono.
- R2. **Dependências:** auditoria de vulnerabilidades conhecidas (Rust e JS) e de licenças das dependências diretas (compatíveis com distribuição proprietária, Lei 19).
- R3. **Excesso de engenharia:** auditoria Ponytail do repositório (código morto, abstrações sem segundo uso, dependências dispensáveis); remover o que for seguro, com testes verdes.
- R4. **Arquitetura:** mapa de dependências entre módulos (Graphify) conferido contra `overview.md`/`contracts.md`; sem dependência circular nem acoplamento fora do desenho.
- R5. **Documentação técnica (AC-016):** AGENTS.md, leis, arquitetura, contratos, specs e roadmap coerentes com o código.

Critério de saída: nenhum achado ALTO aberto; os médios corrigidos ou registrados com justificativa.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-014-01 | Revisão de segurança (R1) → `docs/testing/evidence/security-review.md` | Agente 4 | todo |
| T-014-02 | Auditoria de dependências e licenças (R2) → `docs/testing/evidence/dependency-audit.md` | Agente 1 | todo |
| T-014-03 | Auditoria Ponytail (R3) | Agente 0 | todo |
| T-014-04 | Revisão de arquitetura com Graphify (R4) | Agente 0 | done |
| T-014-05 | Revisão da documentação técnica (R5, AC-016) | Agente 0 | todo |

## STATUS
Criada em 2026-11-20 (coordenador), janela autônoma.
- T-014-04 done (Agente 0, 2026-11-20): `graphify update .` (local, sem LLM): 2.541 nós, 6.193 arestas, 144 comunidades. Nós mais conectados = utilitários e estruturas centrais esperados (errorMessage, useAsync, useNav, useToast, Session, AppState, AnalysisFrame, Reference). Invariantes conferidas por grep:
  - (1) a UI só chama o backend via `lib/ipc.ts` (zero `invoke` fora dele);
  - (2) `scoring-engine` e `game` sem I/O;
  - (3) nenhum crate depende do app;
  - (4) dependências entre crates: `scoring-engine → audio-engine` e `game → scoring-engine` são só `[dev-dependencies]` (testes de regressão do pipeline); a única de produção é `song-analysis → audio-engine + scoring-engine` (orquestração, conforme o desenho);
  - (5) sem mocks em produção (Lei 03).
  Sem dependência circular (o Cargo proíbe; não há ciclo no JS). `graphify-out/` é derivado e fica fora do git.
