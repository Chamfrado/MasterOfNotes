# Teste de atualização assinada (T-009-04)

Executado por: Agente 1, em 2026-11-19, no Windows 11 da máquina de desenvolvimento, com WebView2 157.

**Resultado:** ✅ o instalador por usuário funciona; uma assinatura adulterada é recusada sem instalar nada; uma atualização válida 0.1.0 → 0.1.1 instala, reabre o app e preserva os dados; a desinstalação é limpa.

## Montagem

- **Chave:** descartável, gerada só para este teste (`tauri signer generate`, com senha) no scratchpad da sessão, **fora do repositório**. Nenhum material de chave foi commitado: `git log -p --all | grep -ci "untrusted comment"` → `0`, e a chave pública também não aparece na árvore (`git grep` → 0).
- **Config:** o `tauri.conf.json` do repositório fica sem chave, e por isso o updater fica desligado (T-009-02). O teste usou um overlay `tauri build --config <arquivo no scratchpad>` com:
  - `version` (`0.1.0` ou `0.1.1`);
  - `bundle.createUpdaterArtifacts: true`;
  - `plugins.updater.pubkey` com a chave de teste;
  - `endpoints: ["http://127.0.0.1:8765/latest.json"]`;
  - `dangerousInsecureTransportProtocol: true`, só no overlay: o servidor de teste é HTTP em loopback.
  - O `requireSignedVersion: true` do repositório continuou valendo.
- **Builds release** (LTO), assinadas com `TAURI_SIGNING_PRIVATE_KEY`:

  | Versão | Commit | Instalador | SHA-256 |
  |---|---|---|---|
  | 0.1.0 | `e6f0504` | 3.799.016 bytes | `b5c2cade7d94e9f9bf576e70c4a1668a946612f36c7e992ac3ec769843f5c990` |
  | 0.1.1 | `45dafc4` (main @ 1b38043, UI completa) | 3.805.914 bytes | `0160cb673964debb83c3eaff38902b13dfff653aca2e95eab78c22ca7aa23763` |

  O trusted comment da assinatura da 0.1.1 traz `version:0.1.1`, como exige o `requireSignedVersion`.
- **Servidor:** estático em Node, ouvindo só em `127.0.0.1:8765`. Serviu o instalador da 0.1.1 e o `latest.json`, que foi trocado entre a versão adulterada e a válida.
- **Acionamento:** ainda não havia tela de atualização no build testado. O fluxo foi acionado pelo **IPC real do plugin**, de dentro do WebView2 do app instalado, via CDP:
  - o app foi aberto com `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333 --remote-debugging-address=127.0.0.1`, variável definida só para esse processo;
  - as chamadas foram `plugin:updater|check` e depois `plugin:updater|download_and_install`, as mesmas que o `@tauri-apps/plugin-updater` faz.

## Passos e resultados

1. **Instalar a 0.1.0:** PowerShell `Start-Process … -ArgumentList '/S' -Wait` → `ExitCode=0`.
   - Criou `%LOCALAPPDATA%\Master Of Notes\` (`master-of-notes.exe`, `uninstall.exe`).
   - Criou a entrada `HKCU\…\Uninstall\Master Of Notes` (DisplayVersion `0.1.0`).
   - Criou os atalhos no menu Iniciar e na Área de Trabalho.
   - Não pediu UAC (instalação por usuário).
2. **Criar dados na 0.1.0** pelos commands reais:
   - `set_library_folder` em uma pasta com a música de teste (2 `.webm` indexados, o `.json` ignorado);
   - `create_participant("Teste Update")` (id 1).
3. **Teste negativo:** `latest.json` com um caractere da linha de assinatura do `.sig` trocado (o base64 continua válido).
   - `check` → 0.1.1 disponível (atual 0.1.0).
   - `download_and_install` baixou o pacote (eventos Started/Progress/Finished) e falhou com `The signature verification failed`.
   - Nada foi instalado: o app continuou aberto na 0.1.0 e o registro continuou com 0.1.0.
4. **Atualização válida:** `latest.json` com o `.sig` original.
   - `download_and_install`: o app fechou, o instalador passivo rodou e o app **reabriu sozinho** na 0.1.1.
   - O log do app reaberto mostra a restauração da biblioteca salva: `library scan … found: 2, added: 0`.
   - Registro: `DisplayVersion 0.1.1`; `ProductVersion` do exe: `0.1.1`.
5. **Verificação via IPC na 0.1.1** (reaberta com CDP, porque o processo relançado não herda a variável):
   - `plugin:app|version` → `0.1.1`;
   - `get_settings.libraryFolder` preservado;
   - `list_participants` → `Teste Update` (id 1);
   - `list_songs` → 2 músicas disponíveis;
   - um novo `check` → `null` (nada mais novo).
6. **Desinstalar:** `uninstall.exe /S` → `ExitCode=0`. Depois disso:
   - a pasta de instalação, a entrada em "Aplicativos instalados" e os dois atalhos sumiram;
   - não ficou processo;
   - a pasta de dados `%APPDATA%\com.masterofnotes.karaoke` (com o banco) **permaneceu**, como pede a SPEC-009 R5: dados do usuário ficam fora da instalação;
   - a pasta antiga `%APPDATA%\com.masterofnotes.app` não foi tocada.
7. **Encerramento:**
   - servidor parado; `netstat` sem 8765 e sem 9333 em escuta;
   - `cargo clean --release` (1,9 GiB liberados).

Requisições do servidor (log): `latest.json` + instalador no teste negativo; `latest.json` + instalador na atualização válida; um último `latest.json` no `check` pós-atualização.

## Passeio pela UI no WebView2 real (v0.1.1 instalada, commands reais)

Durante todo o passeio foram capturados `console` error/warning, `pageerror`, `requestfailed` e `Log.entryAdded` do CDP, onde aparecem as violações de CSP. **Nenhum erro, aviso, violação de CSP ou requisição falhou.**

- **Início, Biblioteca:** a lista mostra as 2 músicas, com os botões Cantar/Referência/Editar.
- **Player** (`mon-test-song`):
  - o `<video>` carregou de `http://asset.localhost/…` com a CSP de produção (`readyState` 4, `duration` 31 s, sem erro);
  - "Começar" faz uma contagem regressiva e então toca: `currentTime` 0,96 → 3,97 → 6,99 s;
  - o rótulo "Treino — fora do ranking" está correto, porque a música não tinha referência importada.
- **Microfone:** a tela abre (nível ao vivo, calibração, teste de vazamento) e o medidor recebe frames sem erro. ❓ O nível ficou fixo em −60 (o piso da escala). Isso é compatível com o microfone mudo ou bloqueado pela privacidade, já registrado na SPEC-004; não foi testado com voz.
- **Jogar:** Solo, Festa, Duelo, Desafio, Participantes e Histórico aparecem.
- O diálogo nativo de escolha de pasta não pode ser operado por CDP. A pasta foi definida pelo mesmo command que a tela usa (`set_library_folder`).

## Armadilha registrada: Git Bash e `/S`

No Git Bash (MSYS), um argumento como `/S` é convertido em caminho: `node -e "…argv…" /S` imprime `["S:/"]`. O instalador NSIS então **não** recebe `/S` e abre o assistente interativo. A primeira tentativa travou assim até o timeout, sem instalar nada. Use o PowerShell (`Start-Process -ArgumentList '/S'`) ou `MSYS_NO_PATHCONV=1`.

## Limites

- O transporte foi HTTP em loopback, com `dangerousInsecureTransportProtocol` só no overlay de teste. Em produção o endpoint é HTTPS (GitHub Releases). Isso não foi exercitado porque ainda não há release nem chave do proprietário.
- O fluxo foi acionado por IPC, e não por cliques numa tela de Atualizações: a tela não fazia parte do build testado.
