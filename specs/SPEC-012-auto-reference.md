# SPEC-012 — Referência automática e alinhamento automático

Decisão: [ADR-0004](../docs/adr/ADR-0004-auto-reference.md) frente B. Estudo: [auto-reference-spike.md](../docs/research/auto-reference-spike.md).

## SPEC
- R1. **Modelo de separação baixado pelo usuário.** Em Ajustes → "Referência automática", o app mostra os modelos conhecidos (nome, fonte, licença resumida, tamanho, SHA-256) com o link para a fonte original. O usuário baixa o arquivo `.onnx` **por conta própria**, escolhe o arquivo no app e marca **"Li e aceito os termos da fonte"**. O app confere o SHA-256 contra a lista de conhecidos (**só modelos conhecidos são aceitos**), copia para `app_data_dir/models/` e grava a escolha nas settings. O app **nunca** baixa, distribui ou envia o modelo.
- R2. **Gerar referência do vídeo:** na tela Referência, "Gerar automaticamente" (só com modelo instalado) extrai o áudio do vídeo, separa a voz, transcreve com o nosso YIN + `segment_notes` e salva como referência ativa `source = 'extracted'`, `status = 'experimental'` (a anterior vai para o histórico). **Nunca valida sozinha.**
- R3. **Sem voz:** se o nível da voz separada ficar a mais de ~30 dB abaixo da mistura (limiar do Agente 2, com evidência), nada é salvo: "Este vídeo não tem a voz da melodia (é só instrumental). Use Gravar cantando ou importe um MIDI."
- R4. **Trabalho em segundo plano:** progresso por etapa (extraindo áudio / separando a voz / transcrevendo / salvando) com porcentagem, botão Cancelar, a UI continua usável, um trabalho por vez. Fechar o app cancela.
- R5. **Alinhamento automático:** na tela Referência, "Alinhar automaticamente" calcula o offset entre as notas da referência ativa e o áudio do vídeo (DTW em croma) e **propõe** o valor com a confiança; o usuário vê a prévia e decide se salva (fluxo de offset já existente). Vale para MIDI, JSON e cantada.
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

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-012-01 | `song-analysis`: runtime ONNX + separação em trechos + nível de voz + transcrição + testes (modelo real só em `#[ignore]`) | Agente 2 | todo |
| T-012-02 | `song-analysis`: DTW em croma (`align_offset_ms`) + testes sintéticos | Agente 2 | todo |
| T-012-03 | Commands: modelo (listar conhecidos, instalar com checksum + aceite, remover), upload de áudio, job gerar/cancelar com progresso, alinhar | Agente 1 | todo |
| T-012-04 | UI: Ajustes → Referência automática; Referência → Gerar/Alinhar automaticamente; decodificação do áudio na WebView | Agente 3 | review |
| T-012-05 | fake-backend + E2E + a11y; QA independente da precisão e dos limites | Agente 4 | todo |
| T-012-06 | Guia do usuário (como obter o modelo, termos, limites, revisar e validar) | Agente 5 | todo |

## STATUS
Criada em 2026-11-20 (coordenador), dentro da autorização do proprietário para trabalho autônomo.
- T-012-04 `review` (Agente 3, 2026-11-20):
  - **Ajustes → Referência automática** (R1/R8): explicação simples; para cada modelo conhecido, nome, tamanho, licença, arquivo e SHA-256, mais "Abrir página do modelo" (`openUrl` da fonte). Avisa que o modelo é de terceiros e que o app nunca baixa nem envia o modelo. Checkbox obrigatório "Li e aceito os termos da fonte; o modelo é de terceiros e o uso é de minha responsabilidade.", que habilita "Escolher arquivo baixado…" (diálogo `.onnx`) → `installSeparationModel(path, true)`. Recusas do Rust aparecem em toast. Com modelo instalado: nome, data e "Remover". A rota `settings` aceita `section: "autoReference"`, que foca o título da seção.
  - **Referência → Gerar automaticamente** (R2–R4): sem modelo, leva a Ajustes, já na seção. Com modelo, abre um painel com a explicação e "Começar". Se já existe referência, avisa que a atual vai para o histórico. As etapas aparecem como "Etapa n de 4: Extraindo áudio / Separando a voz / Transcrevendo / Salvando (x%)", com `<progress>` (indeterminado na extração) e Cancelar. Cancelar e sair da tela abortam o fetch e chamam `cancelGenerateReference`.
  - Resultado da geração: a referência volta ao editor, com o selo "Extraída" + experimental e o tempo gasto no toast. `reference: null` mostra o `reason` (R3) e "Tentar de novo". `model_not_installed` oferece "Abrir Ajustes".
  - **Alinhar automaticamente** (R5): botão no cartão do offset. Mostra "Proposta: +X ms (confiança Y%)" e o piano roll já usa o valor, ainda não salvo. "Aplicar" chama `setReferenceOffset`; "Descartar" volta ao offset anterior. Confiança abaixo de 50% mostra um aviso (❓ limiar a calibrar no T-012-05). Se a proposta for igual ao offset atual, só avisa que já confere.
  - **Áudio** (R6, `src/songAudio.ts`): `fetch(videoSrc)` → `decodeAudioData` num `OfflineAudioContext` de 44,1 kHz (reamostra), mono por downmix do Web Audio, `uploadSongAudio`. A referência ao arquivo codificado é largada logo após decodificar. A UI espelha o cache de 1 item do Rust (`{songId, token}`), reusa o token no alinhar e extrai de novo uma única vez em `audio_token_invalid`. Depende da CSP `connect-src asset: http://asset.localhost` (aprovada; aplicada pelo Agente 1).
  - Testes: `songAudio.test.ts` (7), `Settings.test.tsx` (+4), `Reference.test.tsx` (+8).
  - ⏳ Não visto no WebView2 real com o modelo e o backend do T-012-03; decodeAudioData de MP4/MKV no WebView2 ainda não conferido; e2e/fake-backend é o T-012-05.
