# Plano de QA da SPEC-012 (T-012-05)

Agente 4 · 2026-11-20 · base `main` @ ed115e5. Este documento é um plano: os números vão para `song-analysis-qa.md` depois de medidos.

## 1. Fake e testes do contrato (✅ feito)
`e2e/fake-backend.ts` implementa os commands do `ipc.ts` da SPEC-012. O mock (`e2e/tauri-mock.ts`) repassa as `options` do invoke (IPC raw) e aguarda handlers assíncronos.

| Command | Comportamento do fake |
|---|---|
| `list_known_separation_models` | `[knownModel]` (inventado, Lei 19) ou `opts.knownModels` |
| `install_separation_model` | Sem `acceptTerms` → `terms_not_accepted`. Arquivo cujo **nome** não é de um modelo conhecido → `model_unknown` (o fake não calcula hash). Caso contrário grava `settings.separationModel`. |
| `remove_separation_model` | `separationModel = null` |
| `get_settings` / `update_settings` | O que estiver no spec estático (o `setReply` continua valendo), mais o `separationModel` do fake |
| `upload_song_audio` | Corpo `Uint8Array` (f32 LE, tamanho múltiplo de 4, não vazio) e cabeçalho `song-id`; devolve um token novo e invalida o anterior (cache de 1 item, só em memória) |
| `generate_reference` | Exige modelo e o token atual da mesma música; um trabalho por vez (`busy`); sem modelo → `model_not_installed`; token velho → `audio_token_invalid`. Progresso `separating` → `transcribing` → `saving` (0 / 0,5 / 1 em cada) pelo Channel. `opts.generated` decide o resultado: notas, que viram a ativa (`extracted`, `experimental`, revisão 1), ou `"noVoice"`, que encerra depois da separação com a mensagem de R3 sem salvar nada. |
| `cancel_generate_reference` | O trabalho em andamento rejeita com `{code: "cancelled"}` |
| `auto_align_reference` | Exige o token da música da referência; **só propõe** (`opts.alignments`, ou offset atual + 1200 ms com confiança 0,9) |
| `set_reference_offset` | Como o `reference.rs`: ±10 s; se mudou, revisão + 1 e volta a `experimental` |

Códigos decididos no contrato (contracts.md §2.5, coordenador, 2026-11-20), já usados no fake:
- cancelar rejeita com `cancelled`;
- "sem voz" é um resultado `{reference: null, reason}`;
- os demais erros são `terms_not_accepted`, `model_unknown`, `model_not_installed`, `audio_invalid`, `audio_token_invalid`, `busy`, `no_reference` e `not_found`.

`e2e/auto-reference.e2e.ts` exercita esse contrato pelo cliente **real** (`src/lib/ipc.ts` carregado pelo Vite na página), em 3 testes:
1. **Modelo:** lista os conhecidos; recusa a instalação sem aceite e o arquivo desconhecido; instala; o modelo aparece nas settings; remove.
2. **Gerar:**
   - o upload raw leva o cabeçalho `song-id`;
   - o progresso chega na ordem, etapa por etapa;
   - a referência extraída vira a ativa;
   - "sem voz" não salva nada;
   - cancelar no meio dá `cancelled`, e um token velho é recusado.
3. **Gerar sem modelo e alinhar:** gerar sem modelo é recusado. Alinhar só propõe; o offset aceito sobe a revisão e volta a referência a experimental.

## 2. E2E e a11y da UI (✅ escrito sobre a agent-3/work @ 338aad6)
O `useSyntheticVideo(page, ms, audio)` ganhou a trilha Opus de 440 Hz (oscilador → `MediaStreamDestination`), porque a UI decodifica o áudio do vídeo.

**`e2e/auto-reference.e2e.ts`: 6 testes de UI, todos verdes**
1. Ajustes: o aceite é obrigatório; o arquivo desconhecido é recusado; o modelo instala, continua instalado depois de reabrir o app e é removido; "Abrir página do modelo" chama o `open_url` com a URL da fonte.
2. Gerar sem modelo leva a Ajustes, com o foco no título da seção.
3. Gerar com modelo:
   - as etapas aparecem em `role="status"` e o progresso não mexe no foco;
   - o resultado é a referência Extraída, experimental, revisão 1, 5 notas, e ainda é preciso validar à mão;
   - um único upload, com `song-id` e bytes f32 (múltiplo de 4, cerca de 1,5 s);
   - alinhar depois reaproveita o áudio sem novo upload.
4. Sem voz: mostra a mensagem de R3 e a referência anterior continua. Cancelar no meio e sair da tela chamam `cancel_generate_reference`.
5. Alinhar:
   - a proposta aparece com a confiança, e a prévia já move o slider;
   - Descartar não salva;
   - confiança baixa mostra o aviso;
   - Aplicar faz revisão + 1 e volta a referência a experimental.
6. Vídeo sem trilha de áudio: a geração falha com mensagem e oferece "Tentar de novo".

**`a11y.e2e.ts`:** passam pela rodada de axe, teclado, viewports e movimento reduzido as telas Ajustes sem modelo, Ajustes com modelo, Gerar automaticamente em andamento e Proposta de alinhamento.

A sonda de teclado foi corrigida:
- a contagem inclui `video[controls]` e `audio[controls]`;
- o teste percorre uma volta completa (os controles nativos acrescentam paradas de Tab dentro do shadow DOM);
- cada elemento tem uma chave estável;
- o teste detecta contorno de foco **cortado** por um ancestral com overflow.

**Achados de UI (para o Agente 3).** Cada um tem um `test.fail`; quando for corrigido, o teste passa e a marca deve ser retirada.

| ID | Gravidade | Achado | Sugestão |
|---|---|---|---|
| F1 | Média (a11y) | Ao clicar em Começar no painel Gerar, o foco cai no `body`. O `autoFocus` do Cancelar só vale na montagem, e o botão já existia. | Dar foco ao Cancelar por `ref` quando a etapa começar, ou trocar a `key` do botão. |
| F2 | Média (pt-BR, Lei de textos) | Vídeo sem trilha de áudio mostra o erro cru do `decodeAudioData`: "Unable to decode audio data". | Traduzir em `songAudio.ts`, por exemplo: "Não foi possível ler o áudio deste vídeo." |
| F3 | Média (a11y) | O `<video controls>` da prévia de alinhamento (telas Referência e Proposta) recebe foco sem indicador visível: o contorno é cortado pelo `overflow-hidden` do contêiner arredondado. Já existia desde a T-005-04; a sonda antiga não pegava. | `outline-offset` negativo no vídeo, ou tirar o `overflow-hidden` e arredondar o próprio vídeo. |

Ponto de atenção para o Rust (T-012-03): Cancelar e depois Começar logo em seguida não pode dar `busy`. O `cancel_generate_reference` precisa liberar a vaga antes de retornar; a UI não espera o trabalho antigo terminar. O fake já se comporta assim.

### Plano original

**Ajustes → Referência automática**
- Sem modelo:
  - mostra nome, fonte (link), licença, tamanho e SHA-256;
  - avisa que o modelo é de terceiros e a responsabilidade é do usuário (R8);
  - "Instalar" fica bloqueado até marcar "Li e aceito os termos da fonte".
- Arquivo desconhecido: mostra a explicação.
- Instala: o estado aparece; "Remover" volta ao estado sem modelo.

**Referência → Gerar automaticamente**
- Sem modelo: botão desabilitado ou com um caminho claro para Ajustes.
- Com modelo, a geração mostra, em ordem:
  1. etapas com porcentagem (extraindo áudio / separando a voz / transcrevendo / salvando);
  2. a UI continua usável e não abre um segundo trabalho;
  3. no fim, a prévia das notas sobre o vídeo, com os selos "Extraída", Referência experimental e N notas.
- "Validar" continua manual (R2: nunca valida sozinha).
- Sem voz: a mensagem de R3, e a referência anterior continua.
- Cancelar: volta ao estado anterior e chama `cancel_generate_reference`.
- Sair da tela no meio: cancela.
- Upload único por música (cache): gerar e depois alinhar não reenviam o áudio.
- Erro de decodificação (vídeo sem áudio): mensagem.

**Referência → Alinhar automaticamente**
- Proposta com a confiança.
- A prévia aplica o offset sem salvar.
- "Usar" chama `set_reference_offset` (revisão + 1, experimental); "Descartar" não salva.
- Vale para MIDI, JSON, cantada e extraída.

**a11y** (rodada axe, teclado, viewports e movimento reduzido de `a11y.e2e.ts`): Ajustes → Referência automática (sem modelo e com modelo), Referência com geração em andamento, proposta de alinhamento. O progresso precisa ser anunciado (`role="progressbar"` com nome e valor, ou `aria-live` por etapa) e não pode roubar o foco.

## 3. QA independente do `song-analysis` (⏳ quando a T-012-01/02 chegar)
O harness é um `tests/qa_probe.rs` **local, não commitado** no crate (dono: Agente 2), escrito contra a API de contracts.md §2.5. O rascunho está pronto no meu scratchpad e vai arquivado aqui ao final, como nas rodadas de pontuação.

Todo o áudio é sintético e de autoria própria (Lei 19):
- **Voz:** formantes com troca de vogal por nota, jitter 1 %, shimmer 10 %, drift ±15 c, scoop −80 c, vibrato tardio ±40 c e consoante de ruído no ataque.
- **Banda:** pad I–V–vi–IV **no registro da melodia**, baixo e bateria.
- **Melodia:** passeio em dó maior C4–A4, notas de 180–900 ms, notas repetidas e pausas de frase.

**Métrica:** a mesma do spike. Nota correta = casamento 1-para-1, com onset a ±100 ms e altura a ±50 c. Mede recall, precisão, espúrias e a pontuação simulada de um cantor perfeito contra a referência gerada.

| Sonda | Precisa do modelo | O que mede | Critério |
|---|---|---|---|
| Q1a | não | `transcribe` na voz limpa (teto do pipeline) | referência |
| Q1b | sim | Pipeline completo com voz −10, −6, 0 e +6 dB em relação à banda: corretas, recall, precisão, espúrias, pontuação simulada, tempo | R7: ≥ 95 % de notas corretas (precisão); pontuação simulada ≥ 95 |
| Q2 | sim | `voice_level_db` × `NO_VOICE_DB`: karaokê, **lead instrumental no piano**, silêncio, ruído, voz a 0 dB e a fronteira −15…−30 dB | R3 sem falso "tem voz" no instrumental; onde a fronteira cai |
| Q3a | não | `align_offset_ms` com deslocamento conhecido (−5 s … +9 s) em mistura com voz, karaokê sem voz e lead no piano; intro instrumental de 15 s | ±30 ms (critério da SPEC) |
| Q3b | não | Armadilhas: refrão repetido (ambiguidade de uma frase), música errada (a confiança tem de ser baixa), vídeo 1 % mais lento | Confiança honesta |
| Q4a | não | Vazio, 0,3 s, silêncio, clipping ×8, conteúdo de 8 kHz reamostrado, DC, NaN/Inf | Sem pânico; resultado sensato |
| Q4b | não | 10 min: tempo de `transcribe` e `align` | Registro |
| Q4c | sim | 4 min (R7: ≤ 3 min de relógio) e 10 min de separação; pico de memória (R7: ≤ 1,5 GB, `PeakWorkingSet64` medido de fora); cancelar depois de 1 s (latência); progresso monotônico 0..1; taxas de 22,05 e 48 kHz | R7 e R4 |

**Modelo:** baixado no **meu scratchpad** só depois que o Agente 2 publicar o comando de download e o SHA-256. Um modelo por vez, apagado ao final (disco: cerca de 7 GB livres).

**Build:** só `-p song-analysis` (política de disco).
