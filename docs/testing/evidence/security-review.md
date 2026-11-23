# Revisão de segurança independente (SPEC-014 R1, T-014-01)

Agente 4 · 2026-11-20 · base `main` @ 9877d74 (0.1.4).

**Método:** leitura do código de todos os commands do `src-tauri`, da configuração (CSP, asset protocol, capabilities, updater), dos workflows e do script da DLL, e dos plugins nas versões do `Cargo.lock` (código-fonte no cache do cargo). Sondas onde a dúvida era de comportamento: junctions do Windows (programa `rustc` isolado) e SHAs das actions (`gh api` + `git ls-remote`).

**Modelo de ameaça:** app desktop offline, de um usuário só. Uma fronteira não é considerada: quem já escreve arquivos como o próprio usuário já tem o mesmo poder que o app. As fronteiras que valem são três:
- **WebView → Rust (IPC).** O conteúdo exibido vem de arquivos de terceiros: nomes de vídeo, MIDI, JSON, UltraStar, letras. Um XSS daria à página o poder de chamar os commands.
- **Arquivos de terceiros → parsers.**
- **Cadeia de distribuição:** CI, assinatura, updater, DLL e modelo.

## Veredito
**1 ALTO, já corrigido (A1). Nenhum médio.** Seis baixos (B1–B6) e informativos. A superfície IPC está bem validada:
- caminhos absolutos;
- extensões em lista fechada;
- limites de tamanho;
- enums tipados;
- SQL sempre com parâmetros;
- chaves de settings em lista fechada, e as sensíveis bloqueadas;
- limites de texto e de quantidade.

Não há sink de XSS na UI: nenhum `innerHTML`, `eval` ou `dangerouslySetInnerHTML` fora de testes, e o React escapa o texto. Por isso os baixos que dependem de XSS (B2, B3, B5) são defesa em profundidade.

| ID | Gravidade | Dono | Achado | Correção |
|---|---|---|---|---|
| **A1** | **ALTO**, ✅ **corrigido** (agent-4/work @ 5365a23, a valer a partir da 0.1.5) | Agente 4 (`.github/workflows`) | A **chave privada de assinatura do updater** estava no `env` do job de release, visível para todos os passos: 5 actions de terceiros por tag mutável (`@v4`, `@v2`, `@v0`, `@stable`) e o `pnpm install`. O checkout ainda persistia o `GITHUB_TOKEN` (contents: write) no `.git/config`. O comprometimento de uma única action (há precedente: tj-actions/changed-files, 2025) vazaria a chave. Com ela, um atacante assina uma atualização maliciosa, que o app instala sozinho (`installMode: passive`) em todos os usuários. | Chave só nos 2 passos que a usam; as 7 actions fixadas por SHA de commit (conferido por API e `ls-remote`); `persist-credentials: false`; permissões mínimas por job (release: contents: write; CI: contents: read). Nenhum sinal de comprometimento nas versões usadas; trocar a chave fica como decisão do proprietário, por precaução. |
| B1 | Baixo | Agente 1 (`analysis.rs::install`) | **TOCTOU na instalação do modelo:** o SHA-256 é calculado sobre o arquivo de origem e só depois ele é copiado (`std::fs::copy(src, …)`). Se for trocado entre o hash e a cópia, um conteúdo não verificado é instalado como o modelo conhecido e carregado pelo ONNX Runtime. Exige um processo do mesmo usuário trocando o arquivo no instante certo. | Copiar primeiro para o `.part`, calcular o hash **da cópia**, conferir o tamanho e então renomear. Mesma quantidade de I/O. |
| B2 | Baixo | Agente 1 (`commands.rs::allow_library_files`) | **O asset protocol nunca revoga:** os vídeos liberados continuam liberados depois de removidos da biblioteca ou de trocar a pasta, até reiniciar o app. Um arquivo novo criado no mesmo caminho fica legível pela WebView. | `forbid_file` dos caminhos em `change.removed` e dos da pasta anterior ao trocar. O forbid vence o allow, então um vídeo que volta só é liberado depois de reiniciar (aceitável). |
| B3 | Baixo | Agente 2 (`ultrastar.rs`) / Agente 1 | **Oráculo de leitura via mensagem de erro:** `import_reference` e `list_midi_tracks` aceitam qualquer caminho absoluto `.txt`/`.json`/`.mid` até 5 MB, e o parser UltraStar ecoa até 40 caracteres da linha inválida ("linha não reconhecida: \"…\""). Com XSS, a página leria trechos de qualquer `.txt` do disco. | Não ecoar o conteúdo da linha (o número basta), ou só aceitar caminhos vindos do diálogo. |
| B4 | Baixo | Agente 1 (`reference.rs::read_file`) | **Tamanho checado no `metadata`, leitura sem limite:** o arquivo pode crescer entre o `metadata` e o `read`. | `File::open(...).take(MAX + 1)` e recusar se passar. |
| B5 | Baixo | Agente 1 (`capabilities/default.json`) | **`opener:default` é mais largo que o uso:** concede `reveal_item_in_dir` (qualquer caminho) e abrir qualquer URL `http(s)`/`mailto`/`tel`; a UI só abre 3 URLs `https://github.com/…`. | `opener:allow-open-url` com escopo `https://github.com/**`; sem `reveal_item_in_dir`. |
| B6 | Baixo | Agente 1 (`commands.rs::set_library_folder`) | `set_library_folder` aceita qualquer pasta absoluta vinda da WebView (ex.: `C:\` recursivo): varredura pesada, e todos os vídeos do disco ficam servíveis pelo asset protocol. Com a UI íntegra, a pasta vem do diálogo. | Opcional: recusar a raiz de unidade e as pastas do sistema; ou aceitar só o caminho devolvido pelo diálogo. |

### Informativos (sem ação necessária)
- **Logs** (`%LOCALAPPDATA%\com.masterofnotes.karaoke\logs`):
  - 1 arquivo de até 40 KB (`KeepOne`), só Stdout e arquivo; o console da WebView não vai para o log;
  - contêm os caminhos da biblioteca e da pasta de dados (com o **nome de usuário do Windows**), IDs, contagens e métricas;
  - **não** contêm nomes de participantes, letras, áudio nem o conteúdo de arquivos;
  - vale avisar no guia antes de alguém anexar o log a um pedido de suporte.
- **`ORT_DYLIB_PATH`:** se já estiver no ambiente, é respeitado (a DLL do pacote só entra quando a variável não existe). Só o próprio usuário controla o ambiente; fora do modelo.
- **Modelo instalado não é conferido de novo ao carregar** (o arquivo em app data pode ser trocado por um processo do mesmo usuário). Fora do modelo; custaria cerca de 0,3 s por geração se desejado.
- **Upload raw:** limite de 20 min (cerca de 212 MB de f32); o corpo é recebido inteiro antes da checagem (pico de cerca de 2×). Só a própria página pode enviar.

## O que foi conferido e está correto
| Área | Resultado |
|---|---|
| **Commands: entradas** | Caminhos exigem `is_absolute()`; extensões em lista fechada (`mid/midi/json/txt`; vídeos pela lista do scanner); referência ≤ 5 MB; modelo com tamanho exato antes do hash; enums tipados por serde (status, playable, dificuldade, modo); `update_settings` com lista fechada de chaves e `libraryFolder`/`leakCheck`/`separationModel` bloqueados; nomes ≤ 40 e título/artista ≤ 300; partida ≤ 100 músicas e 20 participantes; histórico 1–500; offset ±10 s; sonda ≤ 500 cliques; duração 0–24 h. |
| **SQL** | Sempre com parâmetros; `format!` só concatena constantes (`SONG_SELECT`, `SELECT`) e nomes de tabela em teste. |
| **Junctions e links simbólicos** | Sonda com `mklink /J` (para fora da biblioteca e para a pasta-pai): `DirEntry::file_type()` dá `is_symlink = true` e `is_dir = false`, então o `walk` **não segue** a junction. Não escapa da pasta, não entra em loop. |
| **Asset protocol** | Escopo estático vazio; liberação arquivo a arquivo só dos vídeos indexados (`available = 1`), com padrões escapados; teste `asset_scope_allows_only_indexed_videos`. Revogação → B2. |
| **Upload raw** | Corpo não vazio, múltiplo de 4, ≤ 20 min, sem NaN/Inf (`audio_invalid`); `song-id` precisa existir na biblioteca; um item só em memória, nunca em disco. |
| **Instalação do modelo** | Exige aceite; caminho absoluto; o destino vem do SHA conhecido (`<sha>.onnx`, sem traversal possível); escrita via `.part` + rename. TOCTOU → B1. |
| **CSP** | `script-src 'self'` (sem `unsafe-eval`/`unsafe-inline` em script); `object-src 'none'`; `frame-ancestors 'none'`; `form-action 'none'`; `base-uri 'self'`; `connect-src` só IPC e asset. O `unsafe-inline` existe só em estilo (padrão Tailwind/React). |
| **Capabilities** | Só a janela `main`; diálogo apenas para abrir (`dialog:allow-open`); fullscreen; updater. Opener → B5. |
| **Updater** | Ativo só com `pubkey`; endpoint `https://github.com/…/latest.json`; `requireSignedVersion: true` (existe no plugin 2.14.1); `allow_downgrades` falso por padrão (não aceita versão menor). |
| **DLL** (`fetch-onnxruntime.mjs`) | URL oficial do NuGet; SHA-256 fixado; falha fechada (qualquer erro ou SHA divergente sai com código ≠ 0); escrita atômica (`.part` + rename); idempotente só se o SHA do arquivo existente conferir. |
| **CI** | `on: push (main)` e `pull_request`, sem `pull_request_target`: PR de fork não recebe secrets nem token de escrita. O CI não usa secrets. |
| **pnpm** | pnpm 10 não executa scripts de instalação das dependências por padrão (não há `onlyBuiltDependencies`). |

## Sondas
- **Junction:** um `rustc` de 25 linhas cria `lib/junction-fora → outside` e `lib/junction-pai → ..` e imprime o `file_type()` de cada uma. Rodado e apagado ao final.
- **SHAs das actions:** `gh api repos/<repo>/commits/<tag>` e `git ls-remote <repo> refs/tags/<tag>^{}`, iguais para as 7. As datas dos commits vão de 2025-03 a 2026-11.
- Não compilei o crate Tauri (disco com cerca de 4 GB livres). O B1 e o B2 vêm da leitura do código e são determinísticos pelo fluxo; a correção sugerida para cada um cabe num teste unitário do dono.
