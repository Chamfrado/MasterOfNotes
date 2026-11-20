# SPEC-012 — Referência automática e alinhamento automático

Decisão: [ADR-0004](../docs/adr/ADR-0004-auto-reference.md) frente B. Estudo: [auto-reference-spike.md](../docs/research/auto-reference-spike.md).

## SPEC
- R1. **Modelo de separação baixado pelo usuário.** Em Ajustes → "Referência automática", o app mostra os modelos conhecidos (nome, fonte, licença resumida, tamanho, SHA-256) com o link para a fonte original. O usuário baixa o arquivo `.onnx` **por conta própria**, escolhe o arquivo no app e marca **"Li e aceito os termos da fonte"**. O app confere o SHA-256 contra a lista de conhecidos (**só modelos conhecidos são aceitos**), copia para `app_data_dir/models/` e grava a escolha nas settings. O app **nunca** baixa, distribui ou envia o modelo.
- R2. **Gerar referência do vídeo:** na tela Referência, "Gerar automaticamente" (só com modelo instalado) extrai o áudio do vídeo, separa a voz, transcreve com o nosso YIN + `segment_notes` e salva como referência ativa `source = 'extracted'`, `status = 'experimental'` (a anterior vai para o histórico). **Nunca valida sozinha.**
- R3. **Sem voz clara:** se o nível da voz separada ficar abaixo de `NO_VOICE_DB = −20 dB` em relação à mistura (medido: −3 dB com voz × −39…−55 dB sem voz), nada é salvo e a mensagem é honesta sobre as duas causas possíveis: "Não encontramos uma voz clara neste vídeo: pode ser só instrumental ou a voz estar baixa demais. Use Gravar cantando ou importe um MIDI." (QA M3). **Voz fraca** (entre −20 e −10 dB): a referência é salva, mas a UI avisa que ela pode ter ficado incompleta e deve ser revisada com cuidado (QA M2).
- R4. **Trabalho em segundo plano:** progresso por etapa (extraindo áudio / separando a voz / transcrevendo / salvando) com porcentagem, botão Cancelar, a tela continua responsiva, um trabalho por vez. **Sair da tela Referência ou fechar o app cancela** (a tela avisa antes e durante; decisão de 2026-11-20: trabalho global entre telas só se o uso real pedir).
- R5. **Alinhamento automático:** na tela Referência, "Alinhar automaticamente" calcula o offset entre as notas da referência ativa e o áudio do vídeo (busca exaustiva de defasagem em croma, ±15 s, refinada por parábola; o ótimo exato para um offset constante. O DTW foi descartado porque resolve diferença de andamento, que um offset não representa) e **propõe** o valor com a confiança; o usuário vê a prévia e decide se salva (fluxo de offset já existente). Vale para MIDI, JSON e cantada.
- R6. **Áudio do vídeo:** decodificado na WebView (cobre exatamente os formatos que o player toca), convertido para mono 44,1 kHz f32 e enviado ao Rust uma vez por música (cache de 1 item). Nada de áudio é salvo em disco.
- R7. **Metas** (máquina de referência; números só depois de medidos): música de 4 min em ≤ 3 min de relógio; pico de memória ≤ 1,5 GB; precisão em voz sintética sobre acompanhamento ≥ 95% de notas corretas (método do spike).
- R8. **Lei 19 / transparência:** a tela mostra que o modelo é de terceiros, com a licença dele e a responsabilidade do usuário; o guia explica que a referência gerada pode ter erros e deve ser revisada.

Critérios:
- Given modelo conhecido + vídeo com voz, When Gerar automaticamente, Then nova referência `extracted/experimental` ativa e prévia sobre o vídeo.
- Given vídeo só instrumental, Then nada salvo e a mensagem de R3.
- Given arquivo `.onnx` desconhecido, Then recusado com explicação.
- Given referência MIDI deslocada 1,2 s, When Alinhar automaticamente, Then proposta de offset a ±30 ms do correto (sintético).

## PLAN
- Crate novo **`crates/song-analysis`** (Agente 2), previsto na arquitetura: separação ONNX (runtime escolhido pelo Agente 2 com evidência: `ort` com DLL empacotada ou `tract` puro), STFT/iSTFT, processamento em trechos com overlap-add, transcrição (reusa `audio-engine::analyze_buffer` + `scoring-engine::segment_notes`), DTW em croma. Sem I/O de rede.
- `src-tauri` (Agente 1): gestão do modelo, upload do áudio (raw IPC), job em thread com Channel de progresso, cancelamento, persistência.
- UI (Agente 3): Ajustes → Referência automática; Referência → Gerar automaticamente / Alinhar automaticamente.
- Contrato: [contracts.md §2.5](../docs/architecture/contracts.md) + `ipc.ts`.

### Decisões de runtime (coordenador, 2026-11-20, com medições do Agente 2)
- **ONNX Runtime:** `ort` (MIT/Apache) em modo **load-dynamic** + `onnxruntime.dll` **oficial da Microsoft** (NuGet `Microsoft.ML.OnnxRuntime` 1.31.1, `runtimes/win-x64/native/onnxruntime.dll`, 17,3 MB, MIT). Medido: ~150 s por música de 4 min, pico de 1,23 GB, +4,5 MB no instalador. A DLL só carrega ao gerar; se faltar, a mensagem de erro é clara (não há crash). Rejeitados: `tract` (~220 s, acima da meta) e o `ort` estático (2,5 GB de pico; importa DirectML/D3D12 na partida do app inteiro).
- **Distribuição da DLL:** **não** é commitada e **não** está na config base (o `tauri-build` exige o resource em toda compilação, inclusive no `cargo test` do CI). Entra **só no build de release**: o `release.yml` roda o script de `src-tauri` que baixa a DLL do NuGet oficial (range requests), **verifica o SHA-256 fixado** (falha se divergir) e passa `bundle.resources` pelo overlay `--config` do release, junto do `createUpdaterArtifacts`. Dev, testes e `pnpm build` local funcionam sem a DLL; nesse caso, "gerar" devolve um erro claro. Build reprodutível pelo hash (Lei 12).
- **Créditos (Lei 19):** o aviso de licença MIT da ONNX Runtime vai num `THIRD-PARTY-NOTICES` empacotado e é citado no guia.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-012-01 | `song-analysis`: runtime ONNX + separação em trechos + nível de voz + transcrição + testes (modelo real só em `#[ignore]`) | Agente 2 | review |
| T-012-02 | `song-analysis`: alinhamento por busca de defasagem em croma (`align_offset_ms`) + testes sintéticos | Agente 2 | review |
| T-012-03 | Commands: modelo (listar conhecidos, instalar com checksum + aceite, remover), upload de áudio, job gerar/cancelar com progresso, alinhar | Agente 1 | review |
| T-012-04 | UI: Ajustes → Referência automática; Referência → Gerar/Alinhar automaticamente; decodificação do áudio na WebView | Agente 3 | review |
| T-012-05 | fake-backend + E2E + a11y; QA independente da precisão e dos limites | Agente 4 | review |
| T-012-06 | Guia do usuário (como obter o modelo, termos, limites, revisar e validar) | Agente 5 | review |
| T-012-07 | `song-analysis`: QA M1 (andamento diferente → confiança ≤ 0,3, busca por metade) + M4 (buffer de trecho reutilizável, pico menor) | Agente 2 | review |

## STATUS
Criada em 2026-11-20 (coordenador), dentro da autorização do proprietário para trabalho autônomo.
- 2026-11-20 · Agente 2 · T-012-01/02 em `review`. O crate `crates/song-analysis` segue a API do contracts.md §2.5. Extra aprovado pelo coordenador: a variante `AnalysisError::Input`. O limiar de confiança 0,5 é da UI (contracts.md §2.5); no crate ele fica só nos testes. Evidência completa em [docs/testing/auto-reference.md](../docs/testing/auto-reference.md).
- T-012-04 `review` (Agente 3, 2026-11-20):
  - **Ajustes → Referência automática** (R1/R8): explicação simples; para cada modelo conhecido, nome, tamanho, licença, arquivo e SHA-256, mais "Abrir página do modelo" (`openUrl` da fonte). Avisa que o modelo é de terceiros e que o app nunca baixa nem envia o modelo. Checkbox obrigatório "Li e aceito os termos da fonte; o modelo é de terceiros e o uso é de minha responsabilidade.", que habilita "Escolher arquivo baixado…" (diálogo `.onnx`) → `installSeparationModel(path, true)`. Recusas do Rust aparecem em toast. Com modelo instalado: nome, data e "Remover". A rota `settings` aceita `section: "autoReference"`, que foca o título da seção.
  - **Referência → Gerar automaticamente** (R2–R4): sem modelo, leva a Ajustes, já na seção. Com modelo, abre um painel com a explicação e "Começar". Se já existe referência, avisa que a atual vai para o histórico. As etapas aparecem como "Etapa n de 4: Extraindo áudio / Separando a voz / Transcrevendo / Salvando (x%)", com `<progress>` (indeterminado na extração) e Cancelar. Cancelar e sair da tela abortam o fetch e chamam `cancelGenerateReference`.
  - Resultado da geração: a referência volta ao editor, com o selo "Extraída" + experimental e o tempo gasto no toast. `reference: null` mostra o `reason` (R3) e "Tentar de novo". `model_not_installed` oferece "Abrir Ajustes".
  - **Alinhar automaticamente** (R5): botão no cartão do offset. Mostra "Proposta: +X ms (confiança Y%)" e o piano roll já usa o valor, ainda não salvo. "Aplicar" chama `setReferenceOffset`; "Descartar" volta ao offset anterior. Confiança abaixo de 50% mostra um aviso (❓ limiar a calibrar no T-012-05). Se a proposta for igual ao offset atual, só avisa que já confere.
  - **Áudio** (R6, `src/songAudio.ts`): `fetch(videoSrc)` → `decodeAudioData` num `OfflineAudioContext` de 44,1 kHz (reamostra), mono por downmix do Web Audio, `uploadSongAudio`. A referência ao arquivo codificado é largada logo após decodificar. A UI espelha o cache de 1 item do Rust (`{songId, token}`), reusa o token no alinhar e extrai de novo uma única vez em `audio_token_invalid`. Depende da CSP `connect-src asset: http://asset.localhost` (aprovada; aplicada pelo Agente 1).
  - Testes: `songAudio.test.ts` (7), `Settings.test.tsx` (+4), `Reference.test.tsx` (+8).
  - ⏳ Não visto no WebView2 real com o modelo e o backend do T-012-03; decodeAudioData de MP4/MKV no WebView2 ainda não conferido; e2e/fake-backend é o T-012-05.
- T-012-05 `doing` (Agente 4, 2026-11-20), parte 1 de 3 pronta.
  - Fake: `e2e/fake-backend.ts` simula os commands novos do `ipc.ts`.
    - Modelo: listar os conhecidos; instalar só um modelo conhecido e com aceite, senão erro em pt-BR; remover.
    - `upload_song_audio`: IPC raw (`Uint8Array` + cabeçalho `song-id`); o `tauri-mock.ts` agora repassa as `options` e aguarda handlers assíncronos.
    - `generate_reference`: progresso por etapa, cancelamento e o caso "sem voz".
    - `auto_align_reference` só propõe; `set_reference_offset` sobe a revisão.
  - Teste: `e2e/auto-reference.e2e.ts` exercita o contrato pelo cliente real `src/lib/ipc.ts` (3 testes).
  - Plano de E2E/a11y da UI e da QA independente em [song-analysis-qa-plan.md](../docs/testing/evidence/song-analysis-qa-plan.md); o harness de sondas está pronto em rascunho.
  - ⏳ E2E/a11y esperam a T-012-04; as sondas esperam a T-012-01/02, e as com modelo esperam o comando de download e o SHA do Agente 2.
  - Códigos de erro do fake alinhados ao contrato (§2.5): cancelled, terms_not_accepted, model_unknown, model_not_installed, audio_invalid, audio_token_invalid, busy, not_found.
- T-012-05, parte 2 de 3 pronta (Agente 4, 2026-11-20): E2E/a11y da UI, escritos sobre um merge local da agent-3/work @ 338aad6.
  - `auto-reference.e2e.ts`: +6 testes de UI cobrindo Ajustes, Gerar (etapas, foco, sem voz, cancelar, sair da tela, sem modelo → Ajustes) e Alinhar (prévia, Descartar, confiança baixa, Aplicar).
  - `a11y.e2e.ts`: +4 telas. A sonda de teclado agora conta controles de mídia, faz uma volta completa e detecta contorno de foco cortado.
  - Playwright chromium: 143 passed.
  - Achados de UI F1 (foco cai no body ao Começar), F2 (erro de decodificação em inglês) e F3 (foco do vídeo da prévia sem contorno visível, já existia), todos médios, ficam como `test.fail` em [song-analysis-qa-plan.md](../docs/testing/evidence/song-analysis-qa-plan.md) §2.
  - ⏳ Parte 3: QA do `song-analysis`.
- 2026-11-20 · Agente 2 · T-012-01/02 em `review`. O crate `crates/song-analysis` segue a API do contracts.md §2.5. Há dois extras, propostos ao coordenador: as variantes `AnalysisError::Input` e `pub const LOW_CONFIDENCE: f32 = 0.5`. Evidência completa em [docs/testing/auto-reference.md](../docs/testing/auto-reference.md).
  - **R7 com o modelo real** (`#[ignore]`):
    - voz-guia sintética: **precisão 98,3%**, recall 77,2%, cantor perfeito 98,2;
    - pico de **1353 MiB**;
    - tempo: ~145–150 s por música de 4 min com a CPU livre, 160–231 s com outros builds na máquina;
    - nível de voz: −3,1 dB com voz, −55 dB sem voz. `NO_VOICE_DB = −20`.
  - **R5:**
    - MIDI deslocado 1,2 s: erro de 17 ms (voz-guia) e 5 ms (melodia instrumental), confiança 0,94–0,95;
    - karaokê sem melodia: confiança ≤ 0,02. Ele não alinha, e diz isso.
    - Foi implementado como busca exaustiva de lag no croma, não DTW: o offset é constante (justificativa no doc).
  - **Limites:**
    - cancelamento entre trechos: pior caso de ~1 trecho, ~3,6 s;
    - voz real sem verdade anotada ⏳.
- T-012-03 em `review` (Agente 1, 2026-11-20): `src-tauri/src/analysis.rs`, mais `settings.rs`, `reference.rs`, `lib.rs` e `tauri.conf.json`.
  - **Modelo (R1):**
    - `list_known_separation_models` lê `song_analysis::known_models()`.
    - `install_separation_model(filePath, acceptTerms)` exige o aceite (`terms_not_accepted`). Confere primeiro o tamanho exato e depois o SHA-256 contra a lista (`model_unknown`; nada é copiado). Roda fora da main thread (`blocking`). Copia para `app_data_dir/models/<id>.onnx` (via `.part` + rename) e grava `settings.separationModel {id, name, acceptedMs}`. Um modelo anterior diferente é apagado.
    - `remove_separation_model` apaga o arquivo e a setting, e é idempotente.
    - `separationModel` é somente leitura em `update_settings` (`invalid_setting`).
  - **Áudio (R6):** `upload_song_audio` é um command raw (`tauri::ipc::Request`): corpo f32 LE mono 44,1 kHz e header `song-id`.
    - `audio_invalid` quando o corpo está vazio, não é múltiplo de 4, passa de 20 min ou tem amostra NaN/Inf. `not_found` quando a música não existe.
    - Guarda 1 item em memória (`AnalysisState`) e devolve um token aleatório. O próximo upload substitui o item. Nunca grava em disco.
    - Token velho ou de outra música dá `audio_token_invalid`. O token continua válido depois do generate.
  - **Geração (R2–R4):** `generate_reference(songId, audioToken, onProgress)` roda em `spawn_blocking` com o progresso `separating` → `transcribing` → `saving` pelo Channel.
    - Um job por vez (`busy`), com guarda que libera mesmo em panic. `cancel_generate_reference` usa um AtomicBool e o job rejeita com `cancelled`, sem salvar.
    - `model_not_installed` quando falta a setting ou o arquivo. `AnalysisError::Model/Input` vira `analysis_failed`, com a mensagem pt-BR do crate.
    - Com `voice_level_db < NO_VOICE_DB`, nada é salvo e a reason é a de R3. Com menos de 3 notas, nada é salvo e vem uma reason própria.
    - Fora isso, `reference::store_notes(…, "extracted", …)`, a mesma inserção do import e da referência cantada: `experimental`, ativa, a anterior vai para o histórico. O retorno é `AutoReferenceOutcome {reference, reason, voiceLevelDb, elapsedMs}`.
  - **Alinhar (R5):** `auto_align_reference(referenceId, audioToken)` passa as notas da referência ativa já deslocadas pelo offset atual (as de antes de 0 são descartadas) para `align_offset_ms` e devolve `AlignProposal {offsetMs = atual + proposta, confidence}`. Nada é salvo. Referência inexistente dá `not_found`; substituída ou sem notas, `invalid_input`.
  - **ONNX Runtime (opção b):** `src-tauri/scripts/fetch-onnxruntime.mjs` (Node puro).
    - Lê só o diretório central e a entrada `runtimes/win-x64/native/onnxruntime.dll` do nupkg oficial 1.31.1 por range requests (~6 MB de 160 MB). Confere o SHA-256 fixado `cb2fa518…92db0c` (falha se divergir) e grava `src-tauri/resources/onnxruntime.dll`, que é gitignored. É idempotente.
    - Testado: download novo em 2 s; a 2ª execução não baixa; um arquivo corrompido é baixado de novo.
    - `THIRD-PARTY-NOTICES.txt` (MIT da ONNX Runtime, Lei 19) em `src-tauri`. O `bundle.resources` fica só no overlay do release (Agente 4, `.github/tauri-release.conf.json`), porque o tauri-build exige o resource em toda compilação e isso quebraria o `cargo test` do CI.
    - Em `run()`, antes de qualquer thread: `ORT_DYLIB_PATH = <resource_dir>/onnxruntime.dll`, só se o arquivo existir e a variável não estiver definida. Fora isso vale o fallback do crate.
  - **CSP (aprovada pelo coordenador):** `connect-src` ganhou `asset: http://asset.localhost`. Justificativa: a UI lê o vídeo com `fetch(videoSrc)` para o `decodeAudioData` (R6). O escopo do asset protocol não mudou: só os vídeos indexados, um a um (`allow_file`). `script-src` continua `'self'`.
  - Testes ✅ `analysis::tests::{install_checks_terms_size_and_hash_then_remove_cleans_up, audio_upload_is_validated_and_kept_once, generated_reference_is_extracted_experimental_and_active, align_uses_notes_as_heard_in_the_video}`. `cargo test -p master-of-notes`: 65 ok, 7 ignorados (com a agent-2/work @ 3dcb936 mesclada). Clippy `-D warnings` limpo.
  - ⏳ Pendentes:
    - build release com o overlay: a DLL e os avisos no instalador ainda não foram verificados, porque o disco tem 2,2 GB livres e a regra pede ≥ 4 GB;
    - geração real com modelo (testes `#[ignore]` do Agente 2).
- T-012-03, complemento (Agente 1, 2026-11-20).
  - **Cancelar e gerar de novo:** um `generate_reference` novo que encontra a vaga ocupada por um job JÁ cancelado espera esse job terminar o trecho atual (limite de 30 s) em vez de dar `busy`. Nunca rodam dois separadores juntos. Só um job vivo (não cancelado) dá `busy`. O job antigo continua rejeitando com `cancelled`. Foi apontado pelo Agente 4, cujo fake já se comportava assim. Teste ✅ `analysis::tests::new_job_waits_for_a_cancelled_one_but_not_for_a_live_one`; `cargo test -p master-of-notes`: 66 ok.
  - **Instalador validado** com `pnpm --filter desktop tauri build --bundles nsis --config <overlay>`. O overlay é igual ao `.github/tauri-release.conf.json` do Agente 4, sem `createUpdaterArtifacts`, porque não há chave de assinatura local.
    - O `7z l` do setup (8,6 MB) lista `master-of-notes.exe`, `onnxruntime.dll` e `THIRD-PARTY-NOTICES.txt` na raiz, ou seja, em `$INSTDIR`, ao lado do .exe.
    - No `installer.nsi` gerado, os dois arquivos estão em `File` e em `Delete` do uninstaller.
    - O merge patch manteve o base: `INSTALLMODE currentUser`, PortugueseBR + English, só NSIS.
    - O instalador não foi executado (instrução do coordenador). Depois, `cargo clean --release`.
- T-012-04 `review` (Agente 3, 2026-11-20):
  - **Ajustes → Referência automática** (R1/R8): explicação simples; para cada modelo conhecido, nome, tamanho, licença, arquivo e SHA-256, mais "Abrir página do modelo" (`openUrl` da fonte). Avisa que o modelo é de terceiros e que o app nunca baixa nem envia o modelo. Checkbox obrigatório "Li e aceito os termos da fonte; o modelo é de terceiros e o uso é de minha responsabilidade.", que habilita "Escolher arquivo baixado…" (diálogo `.onnx`) → `installSeparationModel(path, true)`. Recusas do Rust aparecem em toast. Com modelo instalado: nome, data e "Remover". A rota `settings` aceita `section: "autoReference"`, que foca o título da seção.
  - **Referência → Gerar automaticamente** (R2–R4): sem modelo, leva a Ajustes, já na seção. Com modelo, abre um painel com a explicação e "Começar". Se já existe referência, avisa que a atual vai para o histórico. As etapas aparecem como "Etapa n de 4: Extraindo áudio / Separando a voz / Transcrevendo / Salvando (x%)", com `<progress>` (indeterminado na extração) e Cancelar. Cancelar e sair da tela abortam o fetch e chamam `cancelGenerateReference`.
  - Resultado da geração: a referência volta ao editor, com o selo "Extraída" + experimental e o tempo gasto no toast. `reference: null` mostra o `reason` (R3) e "Tentar de novo". `model_not_installed` oferece "Abrir Ajustes".
  - **Alinhar automaticamente** (R5): botão no cartão do offset. Mostra "Proposta: +X ms (confiança Y%)" e o piano roll já usa o valor, ainda não salvo. "Aplicar" chama `setReferenceOffset`; "Descartar" volta ao offset anterior. Confiança abaixo de 50% mostra um aviso (❓ limiar a calibrar no T-012-05). Se a proposta for igual ao offset atual, só avisa que já confere.
  - **Áudio** (R6, `src/songAudio.ts`): `fetch(videoSrc)` → `decodeAudioData` num `OfflineAudioContext` de 44,1 kHz (reamostra), mono por downmix do Web Audio, `uploadSongAudio`. A referência ao arquivo codificado é largada logo após decodificar. A UI espelha o cache de 1 item do Rust (`{songId, token}`), reusa o token no alinhar e extrai de novo uma única vez em `audio_token_invalid`. Depende da CSP `connect-src asset: http://asset.localhost` (aprovada; aplicada pelo Agente 1).
  - Testes: `songAudio.test.ts` (7), `Settings.test.tsx` (+4), `Reference.test.tsx` (+8).
  - ⏳ Não visto no WebView2 real com o modelo e o backend do T-012-03; decodeAudioData de MP4/MKV no WebView2 ainda não conferido; e2e/fake-backend é o T-012-05.
- T-012-06 `review` (Agente 5, 2026-11-20), sobre integ/013:
  - Texto: 05 (três caminhos, Gerar automaticamente: quando funciona, instalar o modelo, aviso de terceiros com o resumo de `known_models()` (UVR: licença dos pesos não declarada, código MIT, crédito pedido, uso comercial incerto), passo a passo com as 4 etapas e "fique nesta tela", tempo medido ~2,5 min para 4 min e ~1,4 GB de memória (docs/testing/auto-reference.md), revisar e validar; Alinhar automaticamente com Proposta/Aplicar/Descartar). 01: ONNX Runtime (MIT) no instalador da 0.1.3 e `THIRD-PARTY-NOTICES.txt` na pasta do programa (`.github/tauri-release.conf.json`). 10, 12, 13, glossário, README. Rótulos conferidos no código final (Settings.tsx, Reference.tsx, `REASON_NO_VOICE` em analysis.rs); TODOs retirados.
  - Mídia (script `capture.docs.ts`, fake-backend do Agente 4; o wrapper de `invoke` do script agora repassa `options`, necessário ao IPC raw): novas `ajustes-modelo`, `referencia-gerar.gif` (vídeo de teste com voz-guia; legenda: animação encurtada), `referencia-sem-voz`, `referencia-alinhar-auto`; regeneradas porque a tela mudou: `configuracoes` (seção nova) e a página 05 (`referencia-vazia`, `-trilha`, `-alinhar.gif`, `-validar`, `-cantar` png/gif: cabeçalho com Gerar automaticamente, cartão com Alinhar automaticamente). `navegacao-setas.gif` saiu diferente só por ruído e foi descartada. Playwright `-g "05 referência|10 ajustes e modo TV"` ✅ 5/5 (a nova passou após a correção do wrapper); typecheck ✅; verificador de links ✅. Total: 56 arquivos, 11,4 MB.
- T-012-06 (Agente 5, 2026-11-20), sobre integ/013 com `pageUrl`: passo a passo do modelo em três tempos (1. **Abrir página do projeto** e ler créditos e termos; 2. **Baixar o arquivo do modelo**, o download é no navegador; 3. aceitar e **Escolher arquivo baixado…**), em 05 e 10, com o aviso novo de terceiros. Regenerada só `ajustes-modelo.png` (o script ganhou `pageUrl` do `known_models()`); `referencia-gerar.gif`, que o mesmo teste reescreve, foi descartada sem mudança. Playwright `-g "gerar e alinhar"` ✅ 1/1; typecheck ✅; verificador de links ✅.
- T-012-05 `review` (Agente 4, 2026-11-20), parte 3 de 3: QA independente do `song-analysis` (agent-2/work @ 7876130) com o modelo real. Relatório em [song-analysis-qa.md](../docs/testing/evidence/song-analysis-qa.md); sondas arquivadas.
  - **Sem achado ALTO.**
  - Medido:
    - 4 min em 132 s, pico de 1353 MiB, precisão de 99,3 %, pontuação simulada de 96,7;
    - sem falso "tem voz" em karaokê, lead de piano, silêncio e ruído;
    - voz em só 20 % da música dá −10,8 dB e é detectada;
    - armadilhas de alinhamento (refrão parcial, música errada, mesmos acordes, transposta, fora de ±15 s) com confiança ≤ 0,37;
    - cancelamento no meio do trecho em ≤ 2,85 s.
  - **Médios:**
    - M1: andamento 0,25–1 % diferente passa com confiança 0,68–0,88 e erro residual de até 464 ms;
    - M2: voz baixa logo acima do limiar salva uma referência com 28 % das notas, sem aviso;
    - M3: o texto "é só instrumental" aparece quando há voz baixa (o limiar está certo, o texto não);
    - M4: a memória cresce cerca de 30 MiB/min (10 min = 1535 MiB).
  - **Baixos:** B1 (NaN; já recusado no upload do app, só pelo crate direto), B2 (viés de ±31–36 ms no alinhamento, sem efeito na pontuação), B3 (inglês numa mensagem inalcançável).
  - O modelo e a DLL foram apagados ao final.
- QA independente (Agente 4, 2026-11-20): sem achados ALTOS. M2/M3 corrigidos na 0.1.3; M1 (andamento ≠ vídeo) e M4 (memória em 10 min: ~1,5 GB) ficam para a 0.1.4/documentação; B1 já coberto pelo upload (NaN/Inf → audio_invalid). Relatório: docs/testing/evidence/song-analysis-qa.md.
- T-012-06 (Agente 5, 2026-11-20), M2/M3: texto de "sem voz" novo (duas causas) e aviso de voz fraca em 05 e 13-FAQ (o aviso aparece logo depois de gerar, abaixo do cabeçalho, e some ao sair da tela: transitório na 0.1.3, decisão do coordenador); memória até ~1,5 GB em músicas de ~10 min. Mesclado agent-4/work @ 517f6ab (fake com o NO_VOICE_REASON novo). Regenerada só `referencia-sem-voz.png`; as outras saídas do mesmo teste foram descartadas. Playwright `-g "gerar e alinhar"` ✅ 1/1; typecheck ✅; verificador de links ✅.
- 2026-11-20 · Agente 2 · T-012-07 em `review` (QA do Agente 4, achados M1 e M4).
  - **M1:** cada metade das notas é buscada de novo, a ±3 s do lag global. Se as metades divergem mais de 100 ms, a confiança fica ≤ 0,3. A checagem só vale com ≥ 3 s de notas em cada metade.
    - Teste sintético de 120 s: andamento 0,25 / 0,5 / 1% dá confiança 0,30 / 0,30 / 0,25; com 0%, erro de 4 ms e confiança 0,93.
    - O alinhamento normal (±1,2 s e armadilhas) não mudou: os mesmos números.
  - **M4:** cada trecho é montado num buffer reutilizável, em vez de uma cópia da música inteira com padding.
    - Pico em 4 min: 1352 → **1313 MiB**. Em 10 min: 1535 → **1435 MiB**.
    - Crescimento: ~30 → ~20 MiB/min.
    - Saída idêntica: a energia dos vocais bate em 6 casas.
