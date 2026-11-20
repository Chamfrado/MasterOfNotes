# Referência automática e alinhamento: medições (SPEC-012, T-012-01/02)

Autor: Agente 2 · 2026-11-20 · crate `crates/song-analysis`.
Legenda (Lei 17): ✅ medido nesta máquina · ❓ estimado ou não verificado · ⏳ pendente.
Máquina: i5-11400H (6 núcleos / 12 threads), 32 GB, Windows 11.

O material de teste é sintético e de autoria própria (Lei 19). É o mesmo do spike ([auto-reference-spike.md](../research/auto-reference-spike.md)), com 4 min e 378 notas:
- **pop com voz-guia:** voz de formantes a 0 dB do acompanhamento;
- **pop com melodia instrumental:** lead a −3 dB;
- **pop karaokê:** só o acompanhamento.

O modelo e a DLL ficaram só no scratchpad do Agente 2 e foram apagados ao final. Nada disso entra no repositório.

## Resumo R7

| Meta (SPEC-012 R7) | Resultado | |
|---|---|---|
| 4 min em ≤ 3 min de relógio | **~145–150 s** com a CPU livre; 160–231 s com builds de outros agentes rodando ao mesmo tempo (veja a tabela) | ✅ sem concorrência; ❓ com a máquina ocupada |
| Pico de memória ≤ 1,5 GB | **1353 MiB** (processo inteiro, com as 3 músicas de 4 min analisadas em sequência) | ✅ |
| ≥ 95% de notas corretas em voz sintética sobre acompanhamento | **precisão 98,3%** (292 de 297 geradas); recall 77,2%; cantor perfeito contra a referência gerada: **98,2** | ✅ |

## Runtime ONNX (decisão aprovada pelo coordenador)

Medido com o MDX-Net Voc_FT, um trecho do modelo (4 × 3072 × 256, 5,75 s de áudio); 4 min são 42 trechos ✅.

| Opção | s/trecho | 4 min | Pico | Instalador (xz) | Por que não / por que sim |
|---|---|---|---|---|---|
| `tract` 0.23 (Rust puro), 12 threads | 4,5–6,1 | ~220 s | 586 MB | +~5 MB | passa da meta de 3 min |
| `ort` estático (binário da pyke) | 3,5 | ~150 s | 2,5 GB | +~5 MB | o build baixa um `.lib` de 360 MB; o `.exe` passa a importar DirectML, D3D12 e MSVCP140 na partida do app inteiro |
| **`ort` load-dynamic + `onnxruntime.dll` oficial 1.31.1, sem arena/mem-pattern** | 3,6 | ~150 s | 1,23 GB | +4,5 MB (DLL de 17,3 MB) | **escolhido**: a DLL só carrega ao gerar |

As saídas das três opções batem entre si (mesma soma de controle do tensor de saída). Sem o arena de memória, o `ort` cai de 2,5 GB para 1,2 GB de pico, na mesma velocidade.

DLL: NuGet `Microsoft.ML.OnnxRuntime` 1.31.1, `runtimes/win-x64/native/onnxruntime.dll`, 17.253.728 bytes, SHA-256 `cb2fa518b18356617f6f9f460c012501da2819ec7e5c1d80bd8f5de20492db0c`, MIT.

O crate procura a DLL em `ORT_DYLIB_PATH` e, se não achar, em `<pasta do exe>/onnxruntime.dll`. O caminho é sempre absoluto, para nunca pegar a cópia antiga que o Windows guarda no System32. Sem a DLL, `Separator::load` devolve `AnalysisError::Model` com mensagem em pt-BR (teste `missing_runtime_is_a_model_error_in_portuguese`).

## Separação (`Separator::vocals`)

Pré e pós-processamento iguais aos do spike (UVR MDX):
- STFT com n_fft 7680, hop 1024, janela de Hann periódica, centrada, com padding por reflexão;
- trechos de 255 hops, com sobreposição de n_fft/2 de cada lado, descartada;
- compensação de 1,021.

A entrada mono é usada nos dois canais, e a saída é a média dos dois canais.

| Teste (CI, sem modelo) | O que garante | |
|---|---|---|
| `stft_then_istft_returns_the_chunk_below_the_cut_off` | STFT → iSTFT devolve o trecho mantido (erro < 1e-3), abaixo do corte de 17,6 kHz do modelo | ✅ |
| `chunks_tile_the_song_without_gaps` | os trechos cobrem a música sem buraco nem sobra | ✅ |
| `unknown_model_is_refused` | modelo fora da lista não carrega | ✅ |
| `separator_and_errors_can_move_to_a_job_thread` | `Separator` e `AnalysisError` são `Send` | ✅ |

Com o modelo real (`#[ignore]`, `tests/real_model.rs`):

Duas rodadas. Nas duas havia compilações de outros agentes na mesma máquina; na 2ª, elas apareceram em 61% das amostras. O tempo depende da carga: o mesmo trabalho levou de 144 a 231 s.

| Material (240 s) | Separação, 1ª rodada | Separação, 2ª rodada | Transcrição | Voz ÷ mistura | Notas | Corretas / recall | Precisão | Cantor perfeito | |
|---|---|---|---|---|---|---|---|---|---|
| pop com voz-guia | 168 s | 193 s | 3,2–3,3 s | −3,1 dB | 297 | 292 / 77,2% | **98,3%** | **98,2** | ✅ |
| pop com melodia instrumental | 231 s | 160 s | 2,9–3,2 s | −55,8 dB | 0 | — | — | — | ✅ |
| pop karaokê | 144 s | 145 s | 2,7–3,0 s | −54,7 dB | 0 | — | — | — | ✅ |

- Com a CPU livre, o benchmark isolado dá 3,6 s por trecho × 42 trechos ≈ 150 s, o que bate com o karaokê (144–145 s).
- Os resultados de qualidade são idênticos nas duas rodadas (determinísticos).
- Pico de memória do processo: 1353 MiB ✅.

- **Cancelamento:** `cancel` é verificado entre trechos. No teste, o retorno veio 3 ms após o pedido feito entre dois trechos ✅. No pior caso, a espera é de um trecho inteiro, ~3,6 s nesta máquina ❓ (dedução).
- **Progresso:** um passo por trecho, de 0 a 1 (42 passos em 4 min).

## Nível de voz (`voice_level_db`, `NO_VOICE_DB = −20 dB`)

| Material | Voz ÷ mistura | |
|---|---|---|
| pop com voz-guia | −3,1 dB | ✅ |
| pop com melodia instrumental | −55,8 dB | ✅ |
| pop karaokê | −54,7 dB | ✅ |
| Vídeo A do spike (voz real; HTDemucs e MDX) | −5,8 dB | ✅ (spike) |

- O limiar de −20 dB fica no meio do vão entre "com voz" (−3 a −6 dB) e "sem voz" (−37 a −56 dB), com folga de ~15 dB de cada lado.
- Uma voz que aparece em só 20% da música, a −5 dB nesse trecho, dá cerca de −12 dB no total e continua detectada ❓ (cálculo, não medido).
- No vídeo só instrumental, a transcrição também sai com 0 notas.

## Transcrição (`transcribe`)

`analyze_buffer` (o nosso YIN) + `segment_notes` (T-005-07), com um piso adaptado ao stem:
- a separação deixa um resíduo fraco onde ninguém canta;
- por isso, frames mais de 30 dB abaixo do p90 dos blocos de 10 ms do stem não contam como voz. É o piso do spike, e o padrão de −50 dBFS continua valendo.

Teste no CI: `transcribe_ignores_separation_residue_and_finds_the_notes` ✅.

Na voz-guia de 4 min, a transcrição leva ~3 s ✅.

O recall de 77% vem quase todo de notas repetidas na mesma altura que viram uma só, como no spike. Isso não afeta a pontuação: notas perdidas saem da conta, enquanto notas espúrias custariam caro.

## Alinhamento (`align_offset_ms`)

O croma ideal das notas (uma classe de altura por frame) é comparado com o croma da mistura:
- a mistura passa por uma STFT de 186 ms com hop de 10 ms, dobrada em 12 classes de 80 Hz a 5 kHz, sem a média e normalizada por frame, de modo que bateria e silêncio pesam 0;
- todos os deslocamentos de ±15 s são testados;
- antes de procurar o pico, a curva tem descontada a média móvel de ±500 ms, porque os acordes longos puxavam o pico;
- o pico é refinado por parábola.

**Por que não DTW.** O spike usou DTW para alinhar duas *gravações* cujo andamento podia diferir. Aqui a correção é um único offset constante. A busca exaustiva é o ótimo exato do mesmo casamento de croma sob essa restrição, e o formato do pico dá a confiança.

**Confiança:** (pico − melhor rival a mais de 1 s) ÷ pico. Abaixo de 0,5 (limiar de exibição da UI, definido no contracts.md §2.5), a proposta não é confiável.

| Áudio (4 min) | MIDI deslocado | Proposta | Erro | Confiança | Tempo (release) | |
|---|---|---|---|---|---|---|
| pop com voz-guia | −1200 ms | +1217 ms | 17 ms | 0,94 | 2,5 s | ✅ |
| pop com voz-guia | +1200 ms | −1183 ms | 17 ms | 0,94 | 2,6 s | ✅ |
| pop com voz-guia | −4321 ms | +4337 ms | 16 ms | 0,95 | 2,5 s | ✅ |
| pop com melodia instrumental | ±1200 / −4321 ms | | 5–6 ms | 0,95 | 2,7–3,0 s | ✅ |
| pop karaokê (sem melodia) | ±1200 / −4321 ms | errada (−7,7 a −13,2 s) | — | **0,00–0,02** | 2,4–2,9 s | ✅ |

Testes no CI, com uma música sintética de 40 s a 22,05 kHz:

| Caso | Erro | Confiança | |
|---|---|---|---|
| MIDI deslocado −1200 / +1200 / −1234 ms, com voz | ≤ 3 ms | 0,90 | ✅ (critério da SPEC: ±30 ms) |
| Só a voz | 3 ms | 0,90 | ✅ |
| Só acompanhamento | — | 0,07–0,35 | ✅ (abaixo de 0,5) |
| Ruído | — | 0,03 | ✅ |
| Áudio silencioso, sem notas | 0 | 0,00 | ✅ |

**Limite:**
- O alinhamento precisa da melodia audível na mistura, seja voz-guia ou instrumento.
- Num karaokê só com acompanhamento, a progressão de acordes se repete, então vários offsets servem. A resposta honesta é confiança ~0, e a UI deve dizer que não foi possível alinhar com segurança.
- Deslocamentos acima de ±15 s não são procurados.

## Como reproduzir

```sh
# CI (sem modelo nem DLL)
cargo test -p song-analysis

# Com o modelo real: baixe UVR-MDX-NET-Voc_FT.onnx da URL de known_models() (confira o SHA-256)
# e a onnxruntime.dll (script de src-tauri); gere o material com o gerador do spike
# (numpy; saída em f32 LE mono 44,1 kHz + pop_truth.json).
SA_MODEL=.../UVR-MDX-NET-Voc_FT.onnx SA_MATERIAL=.../material ORT_DYLIB_PATH=.../onnxruntime.dll \
  cargo test -p song-analysis --release --test real_model -- --ignored --nocapture --test-threads 1
```

- O teste de alinhamento (`r5_alignment_on_the_four_minute_song`) precisa só de `SA_MATERIAL`.
- Para o pico de memória, acompanhe `PeakWorkingSet64` do processo de teste.

## Não medido ⏳❓

- Voz real com verdade anotada: a mesma lacuna do spike, que precisa de anotação humana.
- Máquinas mais fracas que a de referência.
- O pior caso do cancelamento no meio de um trecho.
- Alinhamento em vídeo real.
