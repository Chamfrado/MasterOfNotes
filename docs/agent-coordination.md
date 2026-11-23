# Coordenação da equipe de agentes

## Mecanismo (verificado em 2026-11-19)

- Agent Teams oficial: não exposto nesta instalação. Usado o equivalente disponível: **sessões Claude Code locais com mensageria cross-session** (`ListAgents` / `SendMessage`). Cada mensagem chega à outra sessão como um novo turno; não há contexto compartilhado — **tudo que importa fica no repositório**.
- Coordenador (Agente 0): sessão `masterofnotes-a2`.

| Agente | Sessão | Worktree | Branch base |
|---|---|---|---|
| 1 — Desktop/infra | `masterofnotes-b6` | `.worktrees/agent-1` | `agent-1/work` |
| 2 — Áudio/pontuação | `masterofnotes-c3` | `.worktrees/agent-2` | `agent-2/work` |
| 3 — UX/UI | `masterofnotes-47` | `.worktrees/agent-3` | `agent-3/work` |
| 4 — Modos/QA | `masterofnotes-d4` | `.worktrees/agent-4` | `agent-4/work` |
| 5 — Documentação do usuário | `masterofnotes-cc` | `.worktrees/agent-5` | `agent-5/work` |

Nenhuma worktree ativa em 2026-11-23 (todas integradas e removidas, regra 7). A coluna Worktree diz onde o coordenador a recria ao atribuir uma tarefa; os nomes de sessão são de 2026-11-19 e podem ter mudado.

Se uma sessão for perdida: abra uma nova sessão Claude Code na raiz do repositório, leia `AGENTS.md`, este arquivo e o roadmap, e avise o coordenador do novo nome de sessão.

## Fluxo

1. Coordenador atribui tarefas (`T-NNN-XX`) por mensagem, referenciando a spec.
2. Agente trabalha **só** na sua worktree/branch e nos arquivos que possui.
3. Commits pequenos na própria branch. Antes de entregar: `git merge main` na sua branch e testes verdes.
4. Agente marca a tarefa como `review` na spec e envia o **relatório de entrega** ao coordenador.
5. Coordenador revisa, roda os testes, faz merge em `main` (`--no-ff`), marca `done` e avisa todos para sincronizarem (`git merge main`).
6. Mudança de contrato: o agente propõe; o coordenador altera `contracts.md`/`ipc.ts` em `main`, roda `pnpm check` (inclui o tsc do e2e) e avisa todos.
7. **Worktree terminada é excluída** (regra do proprietário). Quando o agente conclui todas as tarefas atribuídas àquela worktree, ele avisa no relatório: "worktree livre". Depois do merge, o coordenador roda `git worktree remove .worktrees/agent-N` e `git branch -d agent-N/work`. A próxima atribuição recria a worktree a partir de `main`. Nunca remover enquanto houver trabalho não integrado. No Windows: o agente deve sair da pasta (`cd` para a raiz do repo) ao dizer "worktree livre", porque um shell com cwd dentro dela trava a remoção; se `git worktree remove` falhar por caminhos longos (`node_modules`), o coordenador confere `git log main..branch` vazio e `status` limpo e apaga a pasta.

## Relatório de entrega (agente → coordenador)

```
ENTREGA T-NNN-XX (branch agent-N/work @ <sha>)
Feito: <o que funciona de verdade>
Testes ✅: <comandos executados + resultado resumido>
Pendente ⏳: <o que falta>
Suposições ❓: <o que não foi verificado>
Contratos: <nenhuma mudança | proposta: ...>
Próximo: <o que pretende fazer>
```

## Regras anti-conflito

- Um arquivo, um dono (contracts.md §1).
- `Cargo.lock` e `pnpm-lock.yaml`: conflitos são resolvidos pelo coordenador regenerando o lockfile no merge.
- Dependências novas: Rust → no `Cargo.toml` do seu crate. JS → peça ao Agente 3 (dono de `apps/desktop/package.json`).
- Ninguém faz `push --force`, `reset --hard` em branch alheia, nem commit em `main` exceto o coordenador.
- Remoto (`origin`): push só com autorização do proprietário.

## Orçamento de disco (C: tem pouco espaço livre)
- Só **Agente 1** (na sua worktree) e o **coordenador** (em `main`) compilam o app Tauri / `--workspace`.
- Agentes 2 e 4 compilam só os próprios crates: `cargo test -p <crate>`, `cargo clippy -p <crate> --all-targets`. Agente 3 não usa cargo.
- O perfil dev usa `debug = "line-tables-only"` (raiz `Cargo.toml`).
- Antes de builds grandes, confira o espaço livre com `df -h /c`. Se ficar abaixo de 2 GB, avise o coordenador.
