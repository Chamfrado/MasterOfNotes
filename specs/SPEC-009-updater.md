# SPEC-009 — Distribuição e atualização

## SPEC
- R1. SemVer; versão de referência em `tauri.conf.json`; `Cargo.toml` (workspace) e `package.json` acompanham.
- R2. Instalador Windows NSIS, por usuário (sem admin).
- R3. `tauri-plugin-updater` com endpoint `latest.json` em GitHub Releases e chave pública no config. Chave privada **só** em secret do CI ou arquivo local fora do repositório.
- R4. Verificação ao abrir (se `autoCheckUpdates`) sem bloquear a UI; mostra versão e notas; usuário escolhe Atualizar ou Depois; download com progresso; confirmação antes de instalar e reiniciar; nunca durante uma performance.
- R5. Dados do usuário em `app_data_dir` (fora da instalação) → preservados; migrações com backup (SPEC-001).
- R6. Canais: dev (local), test (pre-release), stable (release). Publicar exige autorização do proprietário.
- R7. Falha de rede/assinatura: mensagem clara, app continua funcionando.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-009-01 | Config NSIS + `tauri build` local verde | Agente 1 | todo |
| T-009-02 | Updater plugin + tela Atualizações | Agente 1 / Agente 3 | todo |
| T-009-03 | Workflow de release (cria draft, não publica) | Agente 4 | todo |
| T-009-04 | Teste de atualização local v0.1.0 → v0.1.1 preservando dados | Agente 4 | todo |

## STATUS
Pendente. Bloqueio previsto: chave de assinatura e secrets do GitHub exigem ação do proprietário.
