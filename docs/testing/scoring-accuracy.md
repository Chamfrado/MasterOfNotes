# Precisão da análise e da pontuação

Dono: Agente 2. Specs: [SPEC-004](../../specs/SPEC-004-microphone.md), [SPEC-006](../../specs/SPEC-006-scoring.md).
Ambiente: Intel Core i5-11400H, 31,8 GB RAM, Windows 11, rustc 1.99.1. Medido em 2026-11-19.

Todos os números abaixo vêm de testes executados e reproduzíveis (áudio gerado em código, sem arquivos binários).
Com microfone físico, só o caminho da captura foi exercitado (seção "Dispositivo real"). Pitch com voz real está pendente (⏳).

## Algoritmo v3 (`ALGORITHM_VERSION = 3`, atual)

Decisões: SPEC-006, seção "Algoritmo v3", a partir da [re-revisão do Agente 4](evidence/scoring-review-v2.md). Formulação: comentário de cabeçalho de `crates/scoring-engine/src/scorer.rs` e STATUS da SPEC-006.

**Reproduzir:** `cargo test -p scoring-engine --test regression -- --nocapture --test-threads=1` (≈ 20 s).
- O golden (`golden_results_are_exact`) fixa 12 fixtures. Às 7 do v2 somam-se vibrato ±75 em voz humana, segurar a nota anterior 150 ms, ligadas de 50 ms com 25 ms de atraso, 10% dos frames perdidos e entrada 400 ms antes.
- A comparação v2 × v3 abaixo rodou o `scorer.rs` do v2 (main @ e9694d4) e o do v3 sobre **o mesmo áudio**, numa sonda local fora do repositório, como nas revisões de QA.

**Voz humana:** vogal /a/ com formantes, jitter 1%, shimmer 10%, drift lento de 15 cents (o modelo do Agente 4). "Centrada" quer dizer drift 0. Dificuldade Normal, salvo indicação.

### Metas do v3: v2 × v3 no mesmo áudio

| Item | Caso | Meta v3 | v2 | v3 | ✅ |
|---|---|---|---|---|---|
| N1 | Centrada, vibrato ±50 c a 5,5 Hz (regressão / música de teste) | ≥ 95 | 91,4 / 91,1 | **100,0 / 100,0** | ✅ |
| N1 | Centrada, vibrato ±75 c | ≥ 92 | 77,4 / 77,5 | **99,9 / 99,9** | ✅ |
| N1 | Centrada, vibrato ±100 c | ≥ 88 | 63,3 / 63,9 | **99,7 / 99,7** | ✅ |
| N2 | Canto perfeito, todos os frames | — | 99,7 | 100,0 | ✅ |
| N2 | Canto perfeito, ~5 / 10 / 15% dos frames não chegam | sem perda (acima de 5% vira treino no `performance.rs`) | 95,6 / 90,6 / 88,1 | **100,0 / 100,0 / 100,0** | ✅ |
| N2 | Só os 100 ms iniciais cantados, resto calado (frames chegam) | continua 0 por frame calado | 10,8 | 10,8 | ✅ |
| N3 | Ligadas de 500 ms, segura a anterior 80 ms (calado 80 ms: v2 91,2 / v3 91,9) | ≈ calado | 82,6 | **89,2** | ✅ |
| N3 | Segura a anterior 150 ms (calado 150 ms: 74,4 / 75,1) | ≈ calado | 61,0 | **72,4** | ✅ |
| N4 | Ligadas de 50 ms, 25 ms atrasadas | ≥ 70 | 2,0 | **100,0** | ✅ |
| N4 | Ligadas de 100 ms, 25 ms atrasadas | ≥ 85 | 84,4 | **99,5** | ✅ |
| A3 | Pausas de 200 ms, atrasado / adiantado 300 ms | simétrico | 54,2 / 64,8 | 57,0 / 58,7 (R 0,00 / 0,01) | ✅ |
| A3 | Pausas de 200 ms, atrasado / adiantado 200 ms | simétrico | 72,0 / 77,1 | 73,1 / 74,8 | ✅ |
| A3 | Entra 190 / 400 ms antes e segura (cortando a anterior) | 400 pior que 190 | 91,8 / 71,9 | 90,8 / 66,6 (R 0,52 / 0,01) | ✅ |
| A3 | Pausas de 100 ms, deslocado −200 / +200 ms | simétrico | 79,7 / 69,8 | 75,9 / 74,1 | ✅ |
| A4 | +90 c constante | ≤ 40 | 38,8 | 39,1 | ✅ |
| A4(b) | Maior queda em 10 c, curva 0–300 c de 1 em 1, melodia da regressão (frames ideais): Easy / Normal / Hard | ≤ 10 | 10,2 / **10,9** / 10,6 | 8,5 / **9,7** / 9,7 | ✅ |
| A5 | Sirene de 1 oitava (voz humana) | ≤ 15 | 9,5 | 9,5 | ✅ |
| A5 | Nota fixa, melhor classe (frames ideais) | ≤ 25 | 24,4 | 22,0 | ✅ |

Os mesmos casos, com senoide e com voz humana, estão afirmados em `tests/regression.rs`: `n1_*`, `n2_*`, `n3_*`, `n4_*`, `a3_short_rests_*` e `a4_constant_detune_on_the_regression_song`.

**Curva do amador** (sonda P9b do Agente 4; música de teste, drift ±20 c + vibrato ±50 c, Normal):

| Desvio constante | 0 | +20 c | +30 c | −40 c |
|---|---|---|---|---|
| v2 | 90,4 | 85,6 | 80,8 | 79,2 |
| v3 | 100,0 | 96,7 | 90,8 | 88,2 |

O v3 tirou o custo do vibrato e deixou só o do desvio constante, que segue as tolerâncias do v2 sem mudança. Fica registrado o gatilho de calibração com voz real: o amador típico subiu de 85–90 para 91–100 no Normal. Com desvio de 30–40 c ainda cai para 88–91. ⏳ Confirmar com gravações reais antes de mexer nas tolerâncias.

### O que mudou e por quê

- **A4(b) 10,9 → 9,7:** a queda de 10,7–10,9 entre 50 e 60 c não vinha da curva de tolerância. Vinha da regra "mais perto deste alvo que do anterior": a +60 c, a nota E4 depois de F4 (semitom descendente) nunca ficava mais perto de E4, ficava sem onset e perdia todo o ritmo. No v3, se o cantor nunca chega mais perto, vale o primeiro frame no alvo.
- **Mediana com oitava:** com voz de formantes, o YIN salta oitavas em trocas rápidas de nota (ligadas de 50 ms liam 48 e 76 no lugar de 60 e 64). A mediana em MIDI escolhia valores de oitavas diferentes: ligadas de 50 ms com 25 ms de atraso davam 46,0 com voz humana e 95,8 com senoide. Os valores agora são dobrados para a oitava do próprio frame antes da mediana; a pontuação já ignorava oitava. Resultado: 100,0.
- **Contorno sem vibrato e ornamentos:** só entram na média valores a até 250 c do frame. Um ornamento curto e distante não contamina os vizinhos afinados, e o contorno não mistura os dois lados do onset (N3).
- **Gate:** a acurácia começa no onset desta nota e termina no onset da próxima. É o espelho do N3: quem adianta a nota seguinte paga no ritmo dela, não de novo no gate desta.

### Tabela v1 (continua valendo no v3)

| Caso | Meta | Score | P | R | S | ✅ |
|---|---|---|---|---|---|---|
| Perfeito (senoide) | ≥ 98 | 100,0 | 1,000 | 1,000 | 1,000 | ✅ |
| Perfeito uma oitava abaixo | ≥ 98 | 100,0 | 1,000 | 1,000 | 1,000 | ✅ |
| +100 cents constante | P ∈ [0,45; 0,55] | 30,2 | 0,464 | 0,000 | 0,000 | ✅ |
| +200 cents constante | P = 0, score ≤ 25 | 0,0 | 0,000 | 0,000 | 0,000 | ✅ |
| Silêncio | 0 | 0,0 | | | | ✅ |
| Ruído branco | ≤ 5 | 0,0 | | | | ✅ |
| Atraso de 200 ms | R ∈ [0,3; 0,6], P ≥ 0,7 | 73,1 | 0,802 | 0,448 | 0,804 | ✅ |
| Só 1 de 10 notas cantada | ≤ 15 | 10,0 | 0,100 | 0,100 | 0,100 | ✅ |
| Perfeito com volume 4× | idêntico | 100,0 | | | | ✅ |
| Vibrato ±50 cents a 6 Hz | ≥ 90 | 100,0 | 1,000 | 1,000 | 1,000 | ✅ |
| Voz + acompanhamento a −12 dB | ≥ 85 | 99,8 | 0,998 | 1,000 | 0,998 | ✅ |
| Glissando de 80 ms entre notas | ≥ 92 | 97,7 | 0,965 | 1,000 | 1,000 | ✅ |

Metas do v2 que continuam valendo, medidas no v3:
- A1, notas ligadas repetidas, semitons e tons: 100,0.
- A6, ligadas de 50, 100 e 250 ms: 100,0. As de 30 ms deram 36,1, abaixo da resolução, só informativo.
- A9, voz humana em 3 registros, Normal e Hard, com e sem sopro: 99,7 a 100,0.
- A2, voz-guia vazando a −20 dB: 100,0. O scorer não resolve isso; o vazamento é barrado pelo teste de vazamento (T-004-06).

**Falta de áudio no aparelho (N2, `CaptureStats.missing_ratio`):** em 3 corridas ao vivo de 1 minuto no microfone Realtek desta máquina (`perf_live_5_minutes_*` com `PERF_MINUTES=1`, release), faltaram **0,57%, 0,16% e 0,40%** do áudio. No T-006-07, as contagens de frames ÷ tempo tocado davam 0,3–1,1%. O limite de 5% fica com margem de cerca de 10× sobre o déficit normal deste aparelho. O "40–46 mil amostras/s" registrado no T-006-03 não se repetiu.

## Algoritmo v2 (`ALGORITHM_VERSION = 2`; histórico, vale para performances gravadas com `algorithmVersion = 2`)

Reproduzir: `cargo test -p scoring-engine --test regression -- --nocapture --test-threads=1` (≈ 35 s).
Pipeline completo, Normal salvo indicação. Os casos marcados "A*" vêm da revisão de QA do Agente 4 ([scoring-review.md](evidence/scoring-review.md)). O modelo de voz humana também vem das sondas dele.
O teste `golden_results_are_exact` fixa o `ScoreResult` exato de 7 fixtures: qualquer mudança de resultado quebra o teste.

### Tabela v1 (continua valendo)

| Caso | Meta | Score | P | R | S | Notas | ✅ |
|---|---|---|---|---|---|---|---|
| Perfeito (senoide) | ≥ 98 | 100,0 | 1,000 | 1,000 | 1,000 | 10/10 | ✅ |
| Perfeito uma oitava abaixo | ≥ 98 | 100,0 | 1,000 | 1,000 | 1,000 | 10/10 | ✅ |
| +100 cents | P ∈ [0,45; 0,55] | 30,1 | 0,464 | 0,000 | 0,000 | 0/10 | ✅ |
| +200 cents | P = 0, ≤ 25 | 0,0 | 0,000 | 0,000 | 0,000 | 0/10 | ✅ |
| Silêncio | 0 | 0,0 | 0 | 0 | 0 | 0/10 | ✅ |
| Ruído branco | ≤ 5 | 0,0 | 0 | 0 | 0 | 0/10 | ✅ |
| Atraso de 200 ms | R ∈ [0,3; 0,6], P ≥ 0,7 | 72,3 | 0,802 | 0,431 | 0,772 | 10/10 | ✅ |
| Só 1 de 10 notas | ≤ 15 | 10,0 | 0,100 | 0,100 | 0,100 | 1/10 | ✅ |
| Volume 4× | idêntico | 100,0 | 1,000 | 1,000 | 1,000 | 10/10 | ✅ |
| Vibrato ±50 cents a 6 Hz | ≥ 90 | 91,6 | 0,938 | 0,876 | 0,876 | 10/10 | ✅ |
| Voz + acompanhamento −12 dB | ≥ 85 | 99,7 | 0,997 | 0,998 | 0,995 | 10/10 | ✅ |
| Glissando de 80 ms | ≥ 92 | 94,5 | 0,960 | 0,920 | 0,909 | 10/10 | ✅ |

### Metas do v2

| Achado | Caso | Meta | Score | P | R | S | ✅ |
|---|---|---|---|---|---|---|---|
| A1 | Ligadas repetidas aos pares (500 ms) | R ≥ 0,95 | 100,0 | 1,000 | 1,000 | 1,000 | ✅ (v1: R 0,844) |
| A1 | Ligadas em semitons alternados | R ≥ 0,95 | 100,0 | 1,000 | 1,000 | 1,000 | ✅ |
| A3 | Atrasado / adiantado 100 ms | R simétrico | 90,1 / 91,9 | 0,900 / 0,920 | 0,903 / 0,915 | | ✅ |
| A3 | Atrasado / adiantado 200 ms | R simétrico | 73,0 / 74,8 | 0,800 / 0,820 | 0,448 / 0,461 | | ✅ |
| A3 | Atrasado / adiantado 300 ms | R simétrico | 56,0 / 57,7 | 0,700 / 0,720 | 0,000 / 0,006 | | ✅ |
| A3 | Entra 190 ms antes e segura | (v1: 93,8, R 0,688) | 90,1 | 1,000 | 0,506 | 1,000 | ✅ |
| A4 | +50 / +75 / +90 / +100 / +125 cents | monótono; +90 ≤ 40 | 74,5 / 51,1 / **37,1** / 30,1 / 18,5 | | | | ✅ |
| A4 | Curva 0–300 cents de 1 em 1 (só Scorer) | monótona; salto ≤ 10 pontos em 10 cents | máx. 9,7; +90 = 37,3 | | | | ✅ (meta original ≤ 5: inviável, ver abaixo) |
| A5 | Sirene de 1 oitava, período 400 ms | ≤ 15 | 9,5 | 0,147 | 0,000 | 0,000 | ✅ (v1: 29,9) |
| A5 | Nota fixa contínua (melhor: F4) | ≤ 15 | 24,4 | 0,293 | 0,119 | 0,200 | ❌ meta inviável com justiça, ver abaixo |
| A6 | Ligadas de 50 ms | ≥ 85 | 100,0 | 1,000 | 1,000 | 1,000 | ✅ (v1: 73,7) |
| A6 | Ligadas de 100 ms | ≥ 92 | 100,0 | 1,000 | 1,000 | 1,000 | ✅ (v1: 82,6) |
| A6 | Ligadas de 250 ms | — | 100,0 | | | | ✅ |
| A6 | Ligadas de 30 ms | — (abaixo da resolução: janela de 43 ms) | 34,1 | 0,444 | 0,136 | 0,173 | informativo |
| A8 | Só os 100 ms iniciais de cada nota chegam (só Scorer) | denominador fixo | P = 0,1 | | | | ✅ (v1: 100) |
| A9 | Voz humana (formantes, jitter 1%, shimmer 10%, drift 15 cents), 3 registros, Normal e Hard | ≥ 95 | 99,6 a 100,0 | | | | ✅ |
| A9 | Idem + sopro SNR 10 dB | ≥ 95 | 99,7 a 100,0 | | | | ✅ |
| A2 | Voz-guia vazando −20 / −30 / −40 dB, usuário calado | (T-004-06) | **99,7** / 0,0 / 0,0 | | | | informativo: o scorer não resolve isso |


Evidências sobre as metas que não fecham:
- **(b) "salto ≤ 5 pontos em 10 cents" é incompatível com (c) "+90 ≤ 40" e com perfeito ≈ 100.** De 0 a 90 cents há 9 passos de 10 cents, e cair de 100 para ≤ 40 exige ≥ 60 pontos, ou seja, ≥ 6,7 por passo. Com o limite em 10 pontos o degrau some (era de 35 pontos em 1 cent), e a curva medida tem salto máximo de 9,7.
- **"Nota fixa ≤ 15":** quem sustenta F4 canta de fato as 2 notas F4 da música (20% da duração) com afinação perfeita. Qualquer pontuação justa dá crédito a essas notas. O resto do score vem das notas vizinhas a 1 semitom (P = 0,46 por nota). Para baixar dos 24,4 seria preciso tirar crédito de notas bem cantadas, o que contradiz "Só 1 de 10 notas" = 10. O v2 já tira o ritmo dessas notas quando o cantor não "chega" nelas (ele já estava na altura antes da nota). A regressão afirma ≤ 25.

## Algoritmo v1 (histórico; vale para performances gravadas com `algorithmVersion = 1`)

### Tabela de aceite da SPEC-006 (pipeline completo)

Caminho completo: áudio sintético a 48 kHz → `audio_engine::analyze_buffer` → `Scorer` (Normal).
Tempo de cada frame = centro da janela de análise.
Música de teste: 10 notas de 1000 ms com pausas de 200 ms, começando em 500 ms, melodia C4 D4 E4 F4 G4 A4 G4 F4 E4 D4.
Voz em amplitude 0,2 (≈ −17 dBFS RMS), gate padrão (piso −60 dBFS + 10 dB).

Reproduzir: mesmo comando, no commit `ee22736` (código v1).

| Caso | Esperado (SPEC-006) | Score | P | R | S | Notas acertadas | ✅ |
|---|---|---|---|---|---|---|---|
| Perfeito (senoide na nota) | score ≥ 98 | 100,0 | 1,000 | 1,000 | 1,000 | 10/10 | ✅ |
| Perfeito uma oitava abaixo | score ≥ 98 | 100,0 | 1,000 | 1,000 | 1,000 | 10/10 | ✅ |
| +100 cents constante | P ∈ [0,45; 0,55] | 32,4 | 0,499 | 0,000 | 0,000 | 0/10 | ✅ |
| +200 cents constante | P = 0 e score ≤ 25 | 0,0 | 0,000 | 0,000 | 0,000 | 0/10 | ✅ |
| Silêncio | score = 0 | 0,0 | 0,000 | 0,000 | 0,000 | 0/10 | ✅ |
| Ruído branco (uniforme, pico 0,3, seed fixa) | score ≤ 5 | 0,0 | 0,000 | 0,000 | 0,000 | 0/10 | ✅ |
| Atraso de 200 ms em todas as notas | R ∈ [0,3; 0,6] e P ≥ 0,7 | 73,6 | 0,802 | 0,472 | 0,802 | 10/10 | ✅ |
| Só 1 de 10 notas iguais cantada | score ≤ 15 | 10,0 | 0,100 | 0,100 | 0,100 | 1/10 | ✅ |
| Perfeito com volume 4× | idêntico ao perfeito | 100,0 | 1,000 | 1,000 | 1,000 | 10/10 | ✅ (`ScoreResult` igual bit a bit) |
| Vibrato ±50 cents a 6 Hz | score ≥ 90 | 100,0 | 1,000 | 1,000 | 1,000 | 10/10 | ✅ |
| Voz + acompanhamento a −12 dB | score ≥ 85 | 99,5 | 0,994 | 1,000 | 0,994 | 10/10 | ✅ |
| Perfeito com glissando de 80 ms entre notas | score ≥ 92 | 97,1 | 0,963 | 1,000 | 0,965 | 10/10 | ✅ |
| Extra: perfeito com voz harmônica (8 harmônicos 1/k) | — | 100,0 | 1,000 | 1,000 | 1,000 | 10/10 | ✅ |

Como cada caso foi gerado:
- **Voz + acompanhamento**: voz com 8 harmônicos + tríade maior uma oitava abaixo da nota atual (fundamental, terça, quinta). A tríade se mantém durante a pausa seguinte, e o RMS total dela fica 12 dB abaixo do da voz.
- **Glissando**: cada nota começa na altura da nota anterior e desliza (linear em MIDI) até o alvo nos primeiros 80 ms.
- **Atraso**: cada nota é cantada de `início + 200 ms` até `fim + 200 ms`. O onset medido fica a ≈ 196 ms do início da nota.

Observações:
- **+100 cents** cai exatamente na fronteira p = 0,5. Por isso, sustain e onset (que exigem p ≥ 0,5) dependem de erro subcent da análise e ficam em 0 neste caso. A spec só fixa P, que está correto. Nenhum parâmetro foi ajustado.
- O **vibrato ±50 cents** nunca passa de T_full = 50 cents (Normal). Por isso vale 100, coerente com a fórmula.
- Nenhum parâmetro da fórmula baseline precisou de ajuste: todas as linhas passam com folga.

## Análise de pitch (SPEC-004)

Reproduzir: `cargo test -p audio-engine`

| Critério / caso | Resultado medido |
|---|---|
| Senoide 440 Hz: f0 ∈ [438, 442] em ≥ 95% dos frames | 100% dos frames. Erro máximo 0,07 cents |
| Senoides 75 a 1080 Hz (8 frequências): ±1% | 100% dos frames. Erro máximo 0,19 cents (1080 Hz) |
| Voz sintética 110 Hz (harmônicos 1–10): ±1%, sem erro de oitava | ≥ 95% (teste) |
| Fundamental ausente (harmônicos 2–8 de 110 Hz) | período correto (110 Hz) em ≥ 95% |
| Oitavas 110/220/440/880 Hz (harmônicas) | cada uma no próprio valor, ±1% |
| Glissando exponencial 200→400 Hz em 2 s | ±2% da frequência instantânea em ≥ 95% |
| Silêncio digital | 0 frames vozeados, f0 = None, −120 dBFS |
| Ruído branco (3 seeds, pico 0,3) | 0 frames vozeados (critério: ≤ 1%). Confiança máxima 0,14 |
| −30 vs −6 dBFS (senoide 330 Hz) | mesmo `voiced`; f0 com diferença < 0,01 Hz; confiança com diferença < 1e-3 |
| Clipping (senoide saturada) | ≥ 99% dos frames com `clipped` |
| Tom de 40 Hz (abaixo da faixa) | f0 = None em todos os frames |
| Tom de 2 kHz (acima da faixa) | lido como 1000 Hz, o subharmônico (limitação conhecida do YIN: lags menores que 1/1100 s não são buscados). Inofensivo para a pontuação por causa da dobra de oitava |

## Custo de CPU

`analyze_buffer` sobre 300 s de áudio a 48 kHz (29 996 frames): 8,0 s, ou seja **≈ 2,7% de um núcleo** (meta < 15%).
Medido em build de testes (`opt-level = 3` no perfil dev para este crate) e em release. Os dois deram o mesmo número.

## Dispositivo real (2026-11-19)

`cargo test -p audio-engine --test device -- --ignored --nocapture --test-threads=1`
- ✅ Listagem: "Grupo de microfones (Realtek(R) Audio)" (padrão, 48 kHz, 2 canais), "Microfone (Steam Streaming Microphone)" e "Headset Microphone (Oculus Virtual Audio Device)".
- ✅ Captura de 2 s no dispositivo padrão: 173 frames (≈ 100/s depois da partida do stream), `sample_index` monotônico, sem erro de driver.
- ❓ Piso de ruído medido: −104,8 dBFS. É baixo demais para um microfone embutido, o que sugere microfone mudo ou bloqueado pela privacidade do Windows. Não serve como calibração real.

## Pendente ⏳

- Validação com voz real no microfone (pitch, gate, clipping, calibração): exige o proprietário e entra no checklist em `docs/testing/evidence/`.
- Calibração de latência real (SPEC-004 R7) e vazamento da música para o microfone (R9).
