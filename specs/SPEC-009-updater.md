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
| T-009-01 | Config NSIS + `tauri build` local verde | Agente 1 | done |
| T-009-02 | Updater plugin + tela Atualizações | Agente 1 / Agente 3 | done |
| T-009-03 | Workflow de release (cria draft, não publica) | Agente 4 | done |
| T-009-04 | Teste de atualização local v0.1.0 → v0.1.1 preservando dados (chave de TESTE descartável, servidor local) | Agente 1 | done |

## STATUS
- T-009-03 em review (Agente 4, 2026-11-19): `.github/workflows/release.yml` (tag `v*` ou manual → NSIS + `latest.json` → release **draft** via `tauri-apps/tauri-action`). Nunca executado no GitHub ⏳.
- **Bloqueio do proprietário** (o workflow falha com mensagem explícita até resolver):
  1. Gerar o par de chaves localmente (`pnpm --filter desktop tauri signer generate`); a chave privada fica fora do repositório.
  2. Criar os secrets `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` no GitHub.
  3. Chave pública em `plugins.updater.pubkey` e `bundle.createUpdaterArtifacts: true` no `tauri.conf.json` (Agente 1, T-009-01/02).
- Demais tarefas pendentes.
- T-009-01 `review` (Agente 1, 2026-11-19): `bundle.targets = ["nsis"]`, `installMode: "currentUser"` (sem admin), idiomas PortugueseBR + English. `pnpm build` local verde (release com LTO em 19m50s) → `target/release/bundle/nsis/Master Of Notes_0.1.0_x64-setup.exe` (2,79 MiB). O exe release inicia sem erro (smoke de 12 s). `createUpdaterArtifacts`/`pubkey` aguardam a chave do proprietário. ⏳ Instalação/desinstalação do instalador não testada (fica com T-009-04).
- T-009-01 done (Agente 0, 2026-11-19): `pnpm build` gerou `Master Of Notes_0.1.0_x64-setup.exe` (2,79 MiB, NSIS por usuário, pt-BR/en) na worktree do Agente 1; o binário release sobe sem erro. AC-014 parcial: instalador gerado ✅; instalar/desinstalar ⏳ (T-009-04).
- Decisão (Agente 0, 2026-11-19): identifier `com.masterofnotes.app` → `com.masterofnotes.karaoke`, porque o sufixo `.app` conflita com o bundle do macOS (aviso do Tauri). A troca foi feita **antes de qualquer distribuição**: só a pasta de dados de desenvolvimento desta máquina fica órfã. Depois da primeira release, o identifier é imutável (a pasta `app_data_dir` depende dele, Lei 14).
- Evidência T-009-01 (Agente 1, 2026-11-19): `Master Of Notes_0.1.0_x64-setup.exe`, 2.928.467 bytes, SHA-256 `ec92e7a3f187ee21c855159405a31ac86049d6b9da1fc5591d164d2921bde137`. Gerado de agent-1/work @ 84de723, ainda com o identifier `com.masterofnotes.app`. Depois o identifier mudou para `com.masterofnotes.karaoke` (tauri.conf.json), e `target/release` foi limpo (orçamento de disco). Nenhum código, teste ou evidência tinha o identifier fixo.
- T-009-02 parte backend `review` (Agente 1, 2026-11-19):
  - `tauri-plugin-updater` 2 só é registrado se `plugins.updater.pubkey` não estiver vazio. Sem chave, o app loga "updater desativado: sem chave pública na configuração" e funciona normalmente (R7, Lei 10).
  - O config do repositório traz o endpoint `https://github.com/Chamfrado/MasterOfNotes/releases/latest/download/latest.json`, `requireSignedVersion: true` (anti-downgrade), `windows.installMode: passive` e `pubkey: ""`, aguardando a chave do proprietário.
  - Capability `updater:default`.
  - Teste ✅ `tests::updater_needs_a_real_key`; smoke do app de dev sem chave ✅.
- T-009-04 `review` (Agente 1, 2026-11-19): teste real com chave descartável fora do repo. Evidência: [update-test.md](../docs/testing/evidence/update-test.md). Assinatura adulterada recusada sem instalar; 0.1.0 → 0.1.1 instalou, reabriu e preservou pasta da biblioteca, participante e músicas; desinstalação limpa (dados mantidos). Nenhum material de chave commitado.
- T-009-02, parte da UI (Agente 3, 2026-11-19), em review: `Updates.tsx` (rota `updates`, em Ajustes → "Verificar agora").
  - `check()` mostra a versão instalada (`getVersion`), a nova versão, data e notas. "Baixar atualização" chama `update.download` com progresso em MB. Ao terminar, abre a confirmação "Instalar agora?"; só então `update.install()` roda (o NSIS fecha e reabre o app; sem plugin-process).
  - "Depois" e "Mais tarde" não fazem nada; o recurso Rust é liberado com `close()` ao sair. Só existe fora do Player, então nunca roda durante uma performance (R4).
  - Erro de rede, assinatura ou falta de chave (R7): "Não foi possível verificar ou baixar a atualização. O app continua funcionando normalmente." + detalhe + "Tentar de novo".
  - Ao abrir: `UpdateWatcher` (App) só chama `check()` com `autoCheckUpdates=true` e só avisa por toast; nunca baixa nem instala sozinho.
  - ⏳ Depende do Agente 1: registrar `tauri-plugin-updater` no `lib.rs` e a capability `updater:default`. Sem isso, `check()` falha e a tela mostra o erro.
- T-009-02 e T-009-04 done (Agente 0, 2026-11-19): integrado em main; `pnpm check` exit 0. AC-001 ✅ (instalado e executado no Windows 11, sem UAC), AC-014 ✅ (instalador NSIS), AC-015 ✅ **com chave de teste** (atualização assinada 0.1.0 → 0.1.1 com dados preservados; assinatura adulterada recusada). Evidência: docs/testing/evidence/update-test.md. ⏳ produção: chave do proprietário + secrets + primeiro release.
- T-009-03 done no arquivo (workflow); a execução real no GitHub depende do push/secrets (⏳).
- **Primeiro release (Agente 0, 2026-11-19):** tag `v0.1.0` em `d86976f` (CI verde no GitHub) → workflow Release → rascunho com instalador, `.sig` e `latest.json`, assinatura conferida com a chave `09C130B69909F3B7`. Publicação: proprietário.
- **Atenção (R6, canais):** o endpoint `releases/latest/download/latest.json` ignora pré-releases (`latest` no GitHub = último release normal publicado). Uma versão que deve chegar por atualização automática tem de ser publicada como release **normal**. Pré-releases não chegam por auto-update.
- **v0.1.1 PUBLICADA (Agente 0, 2026-11-19 23:15 UTC) como pré-release**, a primeira versão pública. Tag em `4ad7c26` (CI verde no GitHub); workflow Release ok; instalador 3,8 MB, `latest.json` 0.1.1, assinatura conferida (chave 09C130B69909F3B7). Notas = CHANGELOG + instruções de instalação/SmartScreen. Por ser pré-release, ela não é a `latest` do GitHub: a 1ª versão que deve chegar por auto-update tem de ser publicada como release normal.
- Backlog 0.1.2, tela Atualizações (Agente 3, 2026-11-19, `review`): com só pré-releases publicadas, o endpoint dá 404 e o plugin devolve `ReleaseNotFound` ("Could not fetch a valid release JSON from the remote"). A tela agora mostra "Nenhuma versão nova pelo canal de atualizações automáticas.", com o botão "Abrir página de versões" (`openUrl` das releases do GitHub) e "Verificar de novo". Teste em `Settings.test.tsx`. ❓ O plugin usa a mesma mensagem para qualquer status não-2xx do endpoint (ex.: 500), que então também aparece como "nenhuma versão nova".
- **v0.1.2 PUBLICADA como release NORMAL (Agente 0, 2026-11-20 05:55 UTC)**, a primeira `latest`. Tag em `86d1e04` (CI verde); instalador 3,85 MB; `latest.json` 0.1.2 assinado (09C130B69909F3B7). Endpoint `releases/latest/download/latest.json`: 404 → **200**. A 0.1.1 instalada volta a encontrar atualizações. AC-015 em produção: ✅ **confirmado pelo proprietário em 2026-11-20**: a 0.1.1 instalada encontrou a 0.1.2 pela verificação automática, atualizou e funcionou.
- **v0.1.3 PUBLICADA (release automática, Agente 0, 2026-11-20 08:53 UTC)**: tag em `9cef496` (CI verde); instalador 8,65 MB com `onnxruntime.dll` (SHA-256 extraído do instalador = `cb2fa518…db0c`, igual ao fixado) e `THIRD-PARTY-NOTICES.txt`; `latest.json` 0.1.3 assinado (09C130B69909F3B7); endpoint `latest` = 0.1.3.
- **v0.1.4 PUBLICADA (Agente 0, 2026-11-23)**: tag em `79fd426`; primeiro release com o workflow endurecido (A1) e o `THIRD-PARTY-NOTICES.txt` completo (327 pacotes) no instalador. Assinatura 09C130B69909F3B7 e DLL (`cb2fa518…`) conferidas; endpoint `latest` = 0.1.4.
- **v0.1.5 PUBLICADA (Agente 0, 2026-11-23)**: tag em `c4c3e85`; correções B1, B3–B6 da revisão de segurança. Mesmas conferências; endpoint `latest` = 0.1.5.
- **v0.1.6 PUBLICADA como release normal (Agente 0, 2026-11-23)**, aprovada pelo proprietário. Tag em `4657b7a` (CI verde). `latest.json` 0.1.6 assinado (09C130B69909F3B7); DLL `cb2fa518…` e THIRD-PARTY-NOTICES (13342 linhas) conferidos dentro do instalador (8,7 MB). Endpoint `releases/latest/download/latest.json` → 0.1.6.
- **v0.1.7 PUBLICADA como release normal (Agente 0, 2026-11-24)**, aprovada pelo proprietário. Tag em `4ec592d` (CI verde, incluindo o job `deny`). `latest.json` 0.1.7 assinado (09C130B69909F3B7); DLL `cb2fa518…` e THIRD-PARTY-NOTICES (13342 linhas) conferidos dentro do instalador (8,7 MB). Endpoint `releases/latest/download/latest.json` → 0.1.7.
