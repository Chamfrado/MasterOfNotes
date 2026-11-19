# Contribuindo

1. Leia [AGENTS.md](AGENTS.md) e [docs/project-laws.md](docs/project-laws.md).
2. Toda mudança relevante referencia uma spec/tarefa (`T-NNN-XX`) em `specs/`.
3. Branch a partir de `main`; commits `tipo(escopo): resumo [T-NNN-XX]`.
4. Antes de abrir PR: `pnpm --filter desktop typecheck && pnpm --filter desktop test && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check`.
5. PR descreve o que foi testado (executado vs. pendente).
