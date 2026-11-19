# AGENTS.md — instruções para agentes (atuais e futuros)

**Master Of Notes**: karaokê desktop (Tauri 2 + React + Rust) com pontuação vocal real contra referência melódica. Offline, sem contas.

## Leia antes de qualquer mudança
1. [docs/project-laws.md](docs/project-laws.md) — 20 leis. Obrigatórias.
2. [docs/architecture/overview.md](docs/architecture/overview.md) — camadas, sincronização áudio/vídeo.
3. [docs/architecture/contracts.md](docs/architecture/contracts.md) — **quem é dono de qual arquivo**, APIs dos crates, esquema SQLite, IPC.
4. [specs/](specs/) — a spec da sua tarefa (SPEC/PLAN/TASKS/TEST/STATUS).
5. [docs/agent-coordination.md](docs/agent-coordination.md) — worktrees, branches, mensagens, integração.
6. [docs/roadmap.md](docs/roadmap.md) — fase atual e prioridades.

## Comandos
```sh
pnpm install                                # deps JS (raiz)
pnpm dev                                    # app em modo dev (tauri dev)
pnpm --filter desktop typecheck             # tsc --noEmit
pnpm --filter desktop test                  # vitest
cargo test --workspace                      # testes Rust
cargo test -p scoring-engine                # inclui regressão do algoritmo
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
pnpm build                                  # instalador (tauri build)
```

## Regras de trabalho
- Trabalhe **só** nos arquivos que você possui (contracts.md §1). Precisa mudar arquivo alheio ou contrato? Mensagem ao coordenador.
- Toda tarefa referencia um ID `T-NNN-XX`. Sem tarefa → peça ao coordenador.
- Commits pequenos: `tipo(escopo): resumo [T-NNN-XX]` (tipos: feat, fix, test, docs, refactor, chore, build, ci).
- Pronto = Definition of Done abaixo, não "escrevi o código".
- Ponytail: menos código, stdlib/nativo primeiro, sem abstração especulativa. Nunca à custa de validação, erros, segurança, testes ou precisão do áudio.
- Relatórios separam ✅ executado, ⏳ pendente, ❓ suposição (Lei 17). Nunca declare teste que não rodou.
- Sem mocks no caminho de produção (Lei 03). Mock de IPC só em `*.test.*` / `e2e/`.
- Antes de mudança estrutural: `graphify` (ver CLAUDE.md) + leitura direta do código.

## Definition of Done
1. Spec atualizada (STATUS + tarefa em `review`). 2. Código implementado. 3. Critérios de aceite verificados. 4. Testes executados e verdes (cole a saída resumida no relatório). 5. Sem erro impeditivo conhecido. 6. Leis respeitadas. 7. Interfaces públicas documentadas. 8. Branch atualizada com `main` sem conflito. 9. Revisada pelo coordenador. 10. Roadmap atualizado (coordenador).

## Convenções
- Rust: `thiserror` para erros de crate; `AppError {code, message}` na borda do IPC; sem `unwrap()` fora de testes; `#[serde(rename_all = "camelCase")]` em tipos do IPC.
- TS: strict; sem `any`; todo acesso ao backend via `src/lib/ipc.ts`.
- Textos de UI em pt-BR; código, identificadores e comentários em inglês.
- Fixtures: sintéticas ou de autoria própria; nunca música/vídeo de terceiros (Lei 19).
