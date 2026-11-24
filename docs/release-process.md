# Processo de release

Dono: coordenador (Agente 0). Vale para toda versão publicada.

## Autorização
- 2026-11-20: o proprietário autorizou **releases automáticas** (publicadas pelo coordenador sem pedir confirmação a cada uma) durante a sua ausência de 24 h, desde que todos os portões abaixo passem.
- Fora dessa janela, publicar continua exigindo o OK do proprietário.
- 2026-11-23: o proprietário aprovou publicar a 0.1.4 (UltraStar, avisos de licença, workflow endurecido) e a 0.1.5 (correções de segurança B1, B3–B6), com os mesmos portões.
- 2026-11-23: o proprietário aprovou publicar a 0.1.6 (aviso de falha do microfone; limpeza da auditoria Ponytail), com os mesmos portões.
- 2026-11-23: o proprietário aprovou publicar a 0.1.7 (aviso de voz fraca salvo, `cargo deny` no CI, `cookie`/`url` atualizados, limpezas), com os mesmos portões.

## Portões (todos obrigatórios)
1. **Escopo fechado:** tarefas da versão em `done` nas specs; nada pela metade em `main`.
2. **`pnpm check` verde** na árvore principal: tipos (app + e2e), Vitest, fmt, clippy `-D warnings`, `cargo test --workspace`.
3. **Playwright completo verde** (`CI=1 E2E_PORT=1405`), incluindo `test-song`.
4. **QA independente** (Agente 4) quando a versão mexe em áudio, pontuação, referência ou persistência: relatório sem achado ALTO aberto.
5. **Pontuação:** se `ALGORITHM_VERSION` mudou, exige evidência de voz real (decisão SPEC-006 v5). **Releases automáticas não mudam a pontuação.**
6. **Migração de banco:** se houver uma nova, teste de migração com dados da versão anterior + backup.
7. **Versão** atualizada em `tauri.conf.json`, `Cargo.toml` (workspace) e `apps/desktop/package.json`; **CHANGELOG** escrito para leigos.
8. **CI do GitHub verde** no commit exato da tag.
9. **Workflow Release ok:** instalador + `.sig` + `latest.json` com a versão certa e assinatura da chave `09C130B69909F3B7` (conferida decodificando). Passos só do release (`.github/workflows/release.yml` + `.github/tauri-release.conf.json`): `apps/desktop/src-tauri/scripts/fetch-onnxruntime.mjs` baixa a DLL do ONNX Runtime com SHA-256 fixado e `gen-notices.mjs` (mesma pasta) gera o `THIRD-PARTY-NOTICES.txt` completo; os dois vão para dentro do instalador (conferir).
10. **Publicação como release normal** (`--latest`), com notas = CHANGELOG + "como instalar/atualizar"; depois, endpoint `releases/latest/download/latest.json` = 200 com a versão nova.
11. **Guia do usuário** atualizado (Agente 5) antes ou junto com a publicação.

## Proibido em release automática
- Executar o instalador na máquina do proprietário (há uma instalação dele em uso).
- Mudar a chave, os secrets, a licença ou a visibilidade do repositório.
- Apagar releases ou tags publicadas.
