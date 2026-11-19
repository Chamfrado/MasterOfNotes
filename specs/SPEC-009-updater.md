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
| T-009-01 | Config NSIS + `tauri build` local verde | Agente 1 | review |
| T-009-02 | Updater plugin + tela Atualizações | Agente 1 / Agente 3 | todo |
| T-009-03 | Workflow de release (cria draft, não publica) | Agente 4 | review |
| T-009-04 | Teste de atualização local v0.1.0 → v0.1.1 preservando dados | Agente 4 | todo |

## STATUS
- T-009-03 em review (Agente 4, 2026-11-19): `.github/workflows/release.yml` (tag `v*` ou manual → NSIS + `latest.json` → release **draft** via `tauri-apps/tauri-action`). Nunca executado no GitHub ⏳.
- **Bloqueio do proprietário** (o workflow falha com mensagem explícita até resolver):
  1. Gerar o par de chaves localmente (`pnpm --filter desktop tauri signer generate`); a chave privada fica fora do repositório.
  2. Criar os secrets `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` no GitHub.
  3. Chave pública em `plugins.updater.pubkey` e `bundle.createUpdaterArtifacts: true` no `tauri.conf.json` (Agente 1, T-009-01/02).
- Demais tarefas pendentes.
- T-009-01 `review` (Agente 1, 2026-11-19): `bundle.targets = ["nsis"]`, `installMode: "currentUser"` (sem admin), idiomas PortugueseBR + English. `pnpm build` local verde (release com LTO em 19m50s) → `target/release/bundle/nsis/Master Of Notes_0.1.0_x64-setup.exe` (2,79 MiB). O exe release inicia sem erro (smoke de 12 s). `createUpdaterArtifacts`/`pubkey` aguardam a chave do proprietário. ⏳ Instalação/desinstalação do instalador não testada (fica com T-009-04).
