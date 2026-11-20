# SPEC-012 — Referência automática e alinhamento automático

Decisão: [ADR-0004](../docs/adr/ADR-0004-auto-reference.md) frente B. Estudo: [auto-reference-spike.md](../docs/research/auto-reference-spike.md).

## SPEC
- R1. **Modelo de separação baixado pelo usuário.** Em Ajustes → "Referência automática", o app mostra os modelos conhecidos (nome, fonte, licença resumida, tamanho, SHA-256) com o link para a fonte original. O usuário baixa o arquivo `.onnx` **por conta própria**, escolhe o arquivo no app e marca **"Li e aceito os termos da fonte"**. O app confere o SHA-256 contra a lista de conhecidos (**só modelos conhecidos são aceitos**), copia para `app_data_dir/models/` e grava a escolha nas settings. O app **nunca** baixa, distribui ou envia o modelo.
- R2. **Gerar referência do vídeo:** na tela Referência, "Gerar automaticamente" (só com modelo instalado) extrai o áudio do vídeo, separa a voz, transcreve com o nosso YIN + `segment_notes` e salva como referência ativa `source = 'extracted'`, `status = 'experimental'` (a anterior vai para o histórico). **Nunca valida sozinha.**
- R3. **Sem voz:** se o nível da voz separada ficar abaixo de `NO_VOICE_DB = −20 dB` em relação à mistura (medido: −3 dB com voz × −55 dB sem voz), nada é salvo: "Este vídeo não tem a voz da melodia (é só instrumental). Use Gravar cantando ou importe um MIDI."
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
| T-012-04 | UI: Ajustes → Referência automática; Referência → Gerar/Alinhar automaticamente; decodificação do áudio na WebView | Agente 3 | todo |
| T-012-05 | fake-backend + E2E + a11y; QA independente da precisão e dos limites | Agente 4 | todo |
| T-012-06 | Guia do usuário (como obter o modelo, termos, limites, revisar e validar) | Agente 5 | todo |

## STATUS
Criada em 2026-11-20 (coordenador), dentro da autorização do proprietário para trabalho autônomo.
- 2026-11-20 · Agente 2 · T-012-01/02 em `review`. O crate `crates/song-analysis` segue a API do contracts.md §2.5. Extra aprovado pelo coordenador: a variante `AnalysisError::Input`. O limiar de confiança 0,5 é da UI (contracts.md §2.5); no crate ele fica só nos testes. Evidência completa em [docs/testing/auto-reference.md](../docs/testing/auto-reference.md).
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
