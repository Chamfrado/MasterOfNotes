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

### Decisões de runtime (coordenador, 2026-11-20, com medições do Agente 2)
- **ONNX Runtime:** `ort` (MIT/Apache) em modo **load-dynamic** + `onnxruntime.dll` **oficial da Microsoft** (NuGet `Microsoft.ML.OnnxRuntime` 1.31.1, `runtimes/win-x64/native/onnxruntime.dll`, 17,3 MB, MIT). Medido: ~150 s por música de 4 min, pico de 1,23 GB, +4,5 MB no instalador. A DLL só carrega ao gerar; se faltar, a mensagem de erro é clara (não há crash). Rejeitados: `tract` (~220 s, acima da meta) e o `ort` estático (2,5 GB de pico; importa DirectML/D3D12 na partida do app inteiro).
- **Distribuição da DLL:** **não** é commitada. Um script do build (`src-tauri`) baixa a DLL do pacote NuGet oficial (range requests) e **verifica o SHA-256 fixado**; se o hash não bater, falha. A DLL entra no instalador como resource, ao lado do .exe. Build reprodutível pelo hash (Lei 12).
- **Créditos (Lei 19):** o aviso de licença MIT da ONNX Runtime vai num `THIRD-PARTY-NOTICES` empacotado e é citado no guia.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-012-01 | `song-analysis`: runtime ONNX + separação em trechos + nível de voz + transcrição + testes (modelo real só em `#[ignore]`) | Agente 2 | todo |
| T-012-02 | `song-analysis`: DTW em croma (`align_offset_ms`) + testes sintéticos | Agente 2 | todo |
| T-012-03 | Commands: modelo (listar conhecidos, instalar com checksum + aceite, remover), upload de áudio, job gerar/cancelar com progresso, alinhar | Agente 1 | todo |
| T-012-04 | UI: Ajustes → Referência automática; Referência → Gerar/Alinhar automaticamente; decodificação do áudio na WebView | Agente 3 | todo |
| T-012-05 | fake-backend + E2E + a11y; QA independente da precisão e dos limites | Agente 4 | todo |
| T-012-06 | Guia do usuário (como obter o modelo, termos, limites, revisar e validar) | Agente 5 | todo |

## STATUS
Criada em 2026-11-20 (coordenador), dentro da autorização do proprietário para trabalho autônomo.
