# CLAUDE.md

@AGENTS.md

## Específico do Claude Code

- **Ponytail** (plugin, modo `full`) está ativo: siga a escada (precisa existir? já existe? stdlib? nativo? dependência instalada?) antes de escrever código. `/ponytail-review` em diffs grandes; `/ponytail-audit` na Fase 7.
- **Graphify** (CLI local, `graphify-out/` é derivado e ignorado pelo git): antes de mudança estrutural, rode `/graphify` (ou `graphify update .`) e consulte com `graphify explain "<símbolo>"` / `graphify path "A" "B"`. Não substitui a leitura do código.
- **Coordenação**: as sessões da equipe se comunicam via `SendMessage` (nome da sessão do coordenador está em `docs/agent-coordination.md`). Relatório de entrega no formato de lá.
- **Worktree**: se você é Agente 1–4, trabalhe dentro de `.worktrees/agent-N/` (sua branch). Nunca edite a árvore principal — ela é do coordenador.
- Shell no Windows: Git Bash disponível; prefira caminhos com `/`.
- `pnpm tauri dev` usa a porta 1420 (strict). Só um agente por vez; para validar UI sem Tauri, use `pnpm --filter desktop exec vite --port 14N0` (N = seu número).
