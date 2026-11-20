# Spike: referência melódica automática a partir do vídeo (T-005-06)

Autor: Agente 2 · 2026-11-19 · base `main` @ 2c09278. É um estudo de viabilidade: nenhum código de produto foi escrito. Os protótipos ficaram fora do repositório, no scratchpad do Agente 2. Modelos, venvs, áudio gerado e tudo o que foi derivado do Vídeo A foram apagados; restam só os scripts (alguns KB), até a decisão.
Legenda (Lei 17): ✅ medido nesta máquina · ❓ estimado ou não verificado · ⏳ pendente.
Máquina: i5-11400H (6 núcleos / 12 threads), 32 GB, Windows 11. Todos os tempos são de CPU para **4 min de áudio**, salvo indicação.

## Resposta curta

1. **Tecnicamente dá certo quando o vídeo tem voz-guia.**
   - Pipeline: separação de voz → o nosso YIN → segmentação em notas. Na música sintética com voz de formantes, a referência gerada teve **97–98% de precisão** e **75% de recall** de notas. Um cantor perfeito segundo a melodia real tira **98,2–98,7** contra a referência gerada ✅.
   - Os casos sem voz são reconhecidos com honestidade: 0 notas e uma diferença de energia de mais de 30 dB entre o stem de voz e a mistura ✅.
2. **Hoje não há um modelo de separação que possamos distribuir.**
   - Nenhum modelo de separação tem licença comercial clara dos **pesos**. O Demucs/HTDemucs é efetivamente não comercial, segundo o próprio autor. MDX/UVR, Spleeter e RoFormer não declaram licença dos pesos.
   - O único modelo com licença limpa é o **Basic Pitch** (Apache-2.0, 230 KB). Sem separação, porém, ele gera referências ruins para pontuar: o cantor perfeito tira **70–78** ✅.
3. **"Cantar a referência" funciona sem nenhum modelo.**
   - O usuário canta a melodia uma vez com o microfone, e o nosso YIN + o segmentador geram as notas.
   - Um cantor com ±40 ms de erro de tempo por nota gerou uma referência contra a qual o cantor perfeito tira **98,6** ✅.
   - Sem licença de terceiros, sem modelo, 2,9 s de CPU para 4 min.
4. **O alinhamento por DTW em croma funciona muito bem:** erro mediano de 7 ms e p95 de 16 ms com ajuste linear, em 7 s ✅. Mas ele não resolve a falta de notas: o áudio original também precisa de separação para virar notas.
5. **Voz real ainda é o ponto fraco não medido.** No único vídeo do proprietário (Vídeo A), os dois modelos de separação concordam entre si (85% das notas), mas os dois transcritores (YIN e Basic Pitch) concordam só em ~48%. Não há verdade anotada para dizer qual acerta (detalhes em "Vídeo A").

**Recomendação** (rascunho do ADR-0004 em [ADR-0004-auto-reference.md](../adr/ADR-0004-auto-reference.md), para aprovação do coordenador):
- **Agora (licença limpa):** modo **"Cantar a referência"**, com o motor que já existe. A referência gerada é sempre `experimental` (ADR-0002) e editável.
- **Extração automática do vídeo:** só depois de resolver a licença de um modelo de separação. As saídas são treinar um modelo próprio com dados licenciados ou obter licença escrita dos autores; a decisão é do proprietário. O runtime não é o bloqueio: ONNX via `ort` funciona sem Python nem torch ✅ (caminho ONNX medido em Python; em Rust ❓).
- **Basic Pitch sem separação:** não vale como fonte de referência para modo competitivo. No máximo, um rascunho experimental para vídeo com melodia instrumental.

## Material de teste e limites da amostra

Todo o áudio de teste é sintético e de autoria própria (Lei 19), gerado em numpy:

| Caso | Conteúdo | Verdade |
|---|---|---|
| T-003-05 | Réplica da música de teste: pad de senoides + guia de 3 harmônicos, 31 s, 25 notas | `test-song.json` |
| Pop com voz-guia | 4 min, 378 notas. Voz de formantes (vogais variando, vibrato tardio ±40 c, scoops, portamento, consoantes de ruído, jitter, shimmer, sopro) a 0 dB do acompanhamento. Acompanhamento: piano no registro da melodia (C3–A4), baixo e bateria | melodia gerada |
| Pop com melodia instrumental | Mesmo arranjo; a melodia é tocada por um lead (dente de serra) a −3 dB | idem |
| Pop karaokê | Só o acompanhamento | não há melodia |
| Voz sozinha | A mesma melodia cantada com ±40 ms de erro de tempo em cada borda de nota (o caso "cantar a referência") | idem |
| **Vídeo A** (do proprietário, autorizado) | mp4 H.264 1080p 29,97 fps, AAC 44,1 kHz estéreo, 2:45. **Tem voz:** o stem de voz fica a −5,8 dB da mistura (o karaokê sintético fica a −37 dB). Se é voz-guia ou a gravação original, e o gênero, não determinei: não ouço o áudio e não usei metadados | **nenhuma** |

**Limites (importante):**
- A voz sintética **não é** voz real. A separação reconheceu a voz de formantes como voz (−3,5 dB no stem), mas **não** reconheceu o guia de 3 harmônicos da T-003-05 (−52 dB): por isso a T-003-05 não serve para avaliar o caminho com separação.
- O Vídeo A é **um único vídeo**, sem verdade. Medi só a concordância entre abordagens e fiz uma inspeção visual das notas sobre a curva de f0. Eu não ouço áudio, então não há trecho anotado à mão. Para fechar a precisão em voz real é preciso **um trecho anotado por uma pessoa** (⏳ proprietário) ou músicas com MIDI de autoria própria cantadas por gente.
- Nada do Vídeo A ficou guardado: o áudio extraído, os stems, os frames e as notas foram apagados ao final. O relatório não cita título nem artista.

## Métricas

- **Nota correta:** casamento 1-para-1 entre nota gerada e nota da verdade, com onset dentro de ±100 ms e altura dentro de ±50 c (critério de transcrição, como o `mir_eval`).
  - Recall = corretas ÷ notas da verdade; precisão = corretas ÷ notas geradas.
  - Espúrias = geradas sem par; perdidas = verdade sem par.
- **Pontuação simulada:** frames ideais de um cantor que canta **perfeitamente a melodia real**, pontuados pelo `Scorer` v5 (Normal) contra a referência **gerada**.
  - Responde: "quem canta certo tira quanto se a referência vier dessa abordagem?".
  - Espúrias custam caro (cada nota gerada onde não se canta vale 0); notas perdidas só saem da conta. **Para pontuar, precisão importa mais que recall.**

## Resultados por abordagem

### (1) Vídeo com voz-guia: separação de fontes + o nosso YIN + segmentação

Segmentação (protótipo de ~150 linhas):
- mediana de 5 frames que respeita a oitava;
- corte de nota por silêncio ≥ 30 ms ou por mudança sustentada ≥ 0,6 st por 40 ms;
- notas < 80 ms descartadas;
- piso de energia de 30 dB abaixo do p90 do stem;
- afinação global só quando os desvios são concentrados, com o MIDI fracionário guardado;
- notas iguais separadas por < 20 ms são fundidas.

| Pipeline (pop com voz-guia) | Corretas / recall | Precisão | Espúrias | Pontuação simulada | |
|---|---|---|---|---|---|
| YIN direto na mistura (sem separação) | 98 / 25,9% | 17,0% | 479 | 50,9 | ✅ |
| **HTDemucs → YIN** | 282 / 74,6% | **96,9%** | 9 | **98,2** | ✅ |
| **MDX-Net Voc_FT (ONNX) → YIN** | 288 / 76,2% | **97,6%** | 7 | **98,7** | ✅ |
| HTDemucs → Basic Pitch | 349 / 92,3% | 61,0% | 223 | 95,2 | ✅ |

- **Notas perdidas** (~25%): quase todas são notas repetidas na mesma altura que viram uma só, porque a consoante entre elas é curta demais para o corte de 30 ms. Só afetam o recall, não a pontuação. Dá para melhorar com detecção de onset por energia ❓.
- **Basic Pitch sobre o stem:** acha mais notas, mas quebra o vibrato em notas vizinhas (223 espúrias).
- **Afinação:** a referência gerada deve guardar a afinação da gravação (`midi` fracionário, já previsto no ADR-0002). Quem canta acompanha o vídeo, não o A440. Cuidado: em voz real a média dos desvios é enganosa (Vídeo A: p10/p50/p90 = −40 / +3 / +43 c), então só se aplica um desvio global quando ele é concentrado.

### (2) Instrumental com melodia tocada: Basic Pitch + escolha da linha principal

A linha principal é escolhida por um Viterbi sobre as notas ativas: amplitude, uma leve preferência pelo agudo e custo por salto.

| Caso | Basic Pitch, todas as notas | Linha principal | Pontuação simulada (linha) | |
|---|---|---|---|---|
| Pop com melodia instrumental | recall 91,8%, precisão 10,5% | recall 90,5%, precisão 54,9%, 281 espúrias | **78,3** | ✅ |
| Pop com voz-guia (mistura) | recall 88,9%, precisão 10,2% | recall 87,3%, precisão 43,2%, 434 espúrias | 75,5 | ✅ |
| T-003-05 (mistura) | recall 100%, precisão 29,8% | recall 96%, precisão 68,6% | 69,9 | ✅ |

- O Basic Pitch encontra quase todas as notas da melodia, mas a linha principal também pega notas do piano (que está no mesmo registro) nos trechos em que a melodia descansa. Para pontuar, isso é ruim.
- A separação não ajuda aqui: um lead instrumental cai no stem "outros", junto com o piano. Com o MDX e o HTDemucs, o stem de voz do pop instrumental ficou vazio (−39 a −55 dB).

### (3) Sem melodia: dizer "não foi possível"

| Sinal | Pop com voz-guia | Vídeo A | Pop instrumental | Pop karaokê | |
|---|---|---|---|---|---|
| Energia do stem de voz ÷ mistura (HTDemucs) | −3,5 dB | −5,8 dB | −39,3 dB | −37,5 dB | ✅ |
| Idem (MDX) | −3,1 dB | −5,8 dB | — | −54,7 dB | ✅ |
| Notas geradas (separação → YIN) | 291 | 391 | **0** | **0** | ✅ |
| Basic Pitch, linha principal, sem separação | 764 | — | 623 | **477 (inventadas)** | ✅ |

- **Com separação**, a detecção é robusta. Regra proposta: voz < −20 dB da mistura ou < 20% de frames vozeados no stem → "Não encontramos uma voz-guia neste vídeo".
- **Sem separação**, não há sinal confiável: o Basic Pitch "acha" uma melodia em qualquer acompanhamento.

### (4a) Áudio ORIGINAL fornecido pelo usuário + alinhamento ao vídeo (DTW em croma)

- **Teste:** o "vídeo" é o karaokê atrasado 1,73 s e acelerado 0,6%; o "original" é a versão com voz-guia. As notas da verdade foram mapeadas da linha do tempo do original para a do vídeo.
- **Resultado:** croma CQT com hop de 93 ms, DTW de 2584 × 2588 frames, **7,2 s de CPU** ✅.
  - Mapeando pelo caminho bruto: erro mediano de 27 ms, p95 de 102 ms, máximo de 1,09 s (desvios do caminho).
  - **Com ajuste linear robusto** (atraso + velocidade tirados do caminho): **mediana de 7 ms, p95 de 16 ms** ✅. Velocidade estimada 1,0059 (real 1,006); atraso 1,712 s (real 1,73).
- **Conclusão:** o alinhamento é um problema resolvido, mas o original ainda tem acompanhamento. Transformá-lo em notas cai de volta no item (1), com a mesma questão de licença.
- O mesmo DTW pode ajudar o fluxo que já existe: calcular sozinho o offset de um MIDI ou UltraStar importado contra o vídeo, sintetizando o MIDI. ❓ Não medido.

### (4b) "Cantar a referência": o usuário canta a melodia e salvamos só as notas

| Caso | Corretas / recall | Precisão | Espúrias | Pontuação simulada | |
|---|---|---|---|---|---|
| Voz sozinha, ±40 ms de erro de tempo, avaliada contra a melodia real | 304 / 80,4% | 93,5% | 21 | **98,6** | ✅ |

- **Custo:** nenhum modelo nem licença de terceiros; 2,9 s de CPU (YIN) + ~1 s de segmentação ✅. Usa o mesmo `audio-engine` da performance.
- **Limites:** a referência herda os erros de quem cantou (altura e tempo). Por isso precisa de revisão visual (notas sobre o vídeo) e do status `experimental` até a validação. Medido com voz sintética; com voz real ⏳.

## Vídeo A (voz real; sem verdade: só concordância e inspeção visual)

| Medida | Resultado | |
|---|---|---|
| Stem de voz (HTDemucs / MDX) | −5,8 dB / −5,8 dB da mistura: tem voz | ✅ |
| Notas (HTDemucs → YIN / MDX → YIN) | 391 / 393 | ✅ |
| Concordância HTDemucs × MDX (mesmo transcritor) | 85% das notas; 90% do tempo na mesma classe de altura | ✅ |
| Concordância YIN × Basic Pitch (mesmo stem, mesma grade A440) | ~48% das notas; 60% do tempo na mesma classe de altura | ✅ |
| Afinação global | não concentrada (desvios p10/p50/p90 = −40 / +3 / +43 c) | ✅ |

Inspeção visual de 20 s (f0 do stem + notas das duas abordagens):
- a curva de f0 é limpa e melódica, com deslizes entre notas;
- nas notas sustentadas, YIN e Basic Pitch concordam entre si e com a curva;
- as discordâncias ficam em deslizes, em notas curtas e em **fragmentos do YIN com a oitava errada** (por exemplo, um trecho curto 12 semitons abaixo das vizinhas). Um pós-processamento simples resolve: dobrar para a oitava das notas vizinhas ❓.

**Leitura honesta:** a separação é consistente em voz real; a transcrição em notas de uma voz real (deslizes, ornamentos, sílabas) é onde mora a incerteza. Sem um trecho anotado à mão, não sei dizer a precisão real ⏳.

## Custo (4 min de áudio, CPU desta máquina)

| Etapa | CPU | Relógio | Memória de pico | Modelo | |
|---|---|---|---|---|---|
| O nosso YIN (Rust, já no produto) | 2,9 s | 2,9 s | — | — | ✅ |
| Segmentação em notas (protótipo Python) | ~1 s | | | — | ✅ |
| Basic Pitch (ONNX, Python) | 23 s | 9 s | 259 MB | 230 KB | ✅ |
| HTDemucs (PyTorch CPU, 6 threads) | 707 s | 122 s | 1,5 GB (2:45 de áudio) | 84 MB (+ torch ~770 MB, só no protótipo) | ✅ |
| MDX-Net Voc_FT (ONNX Runtime, sem torch) | 749 s | 133 s | 3,0 GB no protótipo, que segura a música inteira; em trechos deve cair muito ❓ | 63,7 MB | ✅ |
| DTW em croma | 7,2 s | 7,3 s | | — | ✅ |

**Para o usuário:** a separação levaria **~2 min por música de 4 min** nesta máquina, uma vez só por música, com o resultado salvo no banco. Aceitável como etapa de "preparar música" com barra de progresso ❓ (não medi em máquinas mais fracas).

## Licenças (código × pesos)

Fonte: leitura das páginas oficiais (READMEs, LICENSE, cartões de modelo), feita nesta tarefa.

| Modelo | Código | Pesos | Dados de treino | Comercial? |
|---|---|---|---|---|
| **Basic Pitch** (Spotify) | Apache-2.0 | Apache-2.0 (o modelo vem no pacote; LICENSE + NOTICE lidos) | não declarado | **Sim** |
| HTDemucs / htdemucs_ft / mdx_extra | MIT | "The license of the weights is not clear" (o autor, no fork mantido); htdemucs de 6 fontes "likely … CC-NC" | MUSDB18-HQ (uso acadêmico) + dados internos da Meta | **Não, na prática** |
| MDX-Net UVR (Voc_FT, Kim_Vocal_2, KARA) | MIT (UVR GUI) | sem licença; só o pedido "honor the MIT license by providing credit" | não declarado | Incerto |
| MDX23C / BS-RoFormer / Mel-RoFormer (ZFTurbo, Kim, viperx) | MIT (repositórios) | não declarado | não declarado / MUSDB18 | Incerto |
| Spleeter (Deezer) | MIT | não declarado (os mantenedores evitam o assunto) | não declarado | Incerto |
| Open-Unmix umxhq / umxl | MIT | umxhq: MIT (Zenodo), mas treinado em MUSDB18-HQ (acadêmico); umxl: **CC BY-NC-SA** | MUSDB18-HQ / privado | umxhq: risco; umxl: **Não** |
| CREPE / torchcrepe | MIT | não declarado | MIR-1K, MedleyDB e outros (acesso restrito) | Incerto |
| RMVPE, FCPE, SwiftF0, PESTO | Apache / MIT / LGPL | não declarado ou CC BY-NC-SA (FCPE do paper); SwiftF0 treinado em base CC BY-NC-SA | | Incerto ou Não |
| **O nosso YIN** | nosso | — | — | **Sim** |

Runtimes: o crate `ort` é MIT OR Apache-2.0; o `tract` é MIT OR Apache-2.0. A ONNX Runtime é MIT segundo o PyPI ❓; não consegui abrir o LICENSE no GitHub na hora da consulta.

**Lei 19:** a lei proíbe redistribuir modelos de terceiros. O Basic Pitch é Apache-2.0, o que permite redistribuir com aviso, mas cabe ao coordenador dizer se a lei admite isso ou se o modelo deve ser baixado sob consentimento. Para modelos de separação, nenhum atende hoje.

## Runtime no produto (Rust)

- **Basic Pitch:** o `nmp.onnx` tem 230 KB e roda no ONNX Runtime pelo `ort`. O `tract`, em Rust puro, talvez rode ❓ (não testei os operadores). A entrada é um CQT harmônico calculado dentro do modelo: só é preciso entregar áudio mono a 22 050 Hz ❓ (verificar na integração).
- **Separação MDX-like:** ONNX + STFT/ISTFT (n_fft 7680, hop 1024, recorte em trechos com sobreposição). Em Rust: `rustfft` + `ort`. Medi o caminho completo **sem torch** em Python ✅; em Rust ❓. O ONNX Runtime acrescenta ~10–20 MB de DLL ao instalador ❓.
- **Demucs:** exige PyTorch, ou uma conversão para ONNX que eu não testei. Não serve para o produto também por causa da licença.

## O que não foi medido ⏳❓

- Precisão em voz real contra uma verdade anotada (precisa de anotação humana).
- Ruído de sala, reverberação forte, backing vocals e harmonias (o stem de voz traz o coro junto).
- Máquinas mais fracas, e o tempo e a memória do caminho ONNX implementado em Rust e processado em trechos.
- Detecção de onset por energia, para separar notas repetidas.
- DTW para ajustar o offset de MIDI importado.
- O `tract` com o modelo do Basic Pitch.

## Como reproduzir

Os modelos e os venvs foram apagados, como pedido; os scripts continuam no scratchpad do Agente 2. O que eles fazem:
- `gen.py`: gera o material sintético acima;
- harness Rust: chama `audio_engine::analyze_buffer` e o `Scorer` v5;
- `notes.py`: segmentação + métricas;
- `bp_run.py`: Basic Pitch ONNX + linha principal;
- `demucs_run.py`: HTDemucs;
- `mdx_run.py`: MDX via onnxruntime + STFT em numpy;
- `dtw_align.py`: croma CQT + DTW do librosa + ajuste linear.

Se o ADR for aceito, o segmentador e as métricas viram código de produto com testes. Os demais protótipos dependem da decisão de licença.
