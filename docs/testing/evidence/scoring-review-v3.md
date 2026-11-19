# 3ª rodada de QA independente: pontuação v3 (T-006-10)

Revisor: Agente 4 · 2026-11-19 · `main` @ 7caa1c9 · `ALGORITHM_VERSION = 3`. Nada em `src/` foi alterado.
**Método:** as 16 sondas rodam em **v2 × v3 sobre o mesmo áudio**: as 14 da rodada anterior mais P15 e P16, novas. O v2 é o `scorer.rs` @ ff25eea, incluído como módulo. A cópia arquivada está em [scoring-review-v3-probes.rs](scoring-review-v3-probes.rs), com as instruções no cabeçalho. 16/16 executadas, cerca de 130 s.
Células: `score (P/R/S) acertos`. Normal, salvo indicação. "Frames ideais" = só o Scorer; "pipeline" = áudio → `analyze_buffer` → Scorer.

## Resumo

| Item | Meta v3 | Medido v2 → v3 | Situação |
|---|---|---|---|
| N1 vibrato (voz realista centrada) | ±50 ≥ 95 · ±75 ≥ 92 · ±100 ≥ 88 | 91,1 → **100,0** · 77,5 → **99,9** · 63,9 → **99,7** | ✅ |
| N2 frames ausentes | sem punição abaixo de 5 % | 5 / 10 / 15 % ao acaso: 96,0 / 91,0 / 88,1 → **100 / 100 / 100**; no aparelho faltam 0,16–0,57 % (scoring-accuracy.md) | ✅ |
| N3 atraso legato cobrado uma vez | — | segura a anterior 150 ms: 60,8 → **72,6** (R 0,34 → 0,67 = só o ramp de 150 ms; S 0,38 → 0,74). 80 ms: 82,2 → 89,5 | ✅ |
| N4 notas curtas × latência | 50 ms +25 ms ≥ 70 · 100 ms +25 ms ≥ 85 | 1,1 → **95,8** · 72,3 → **99,5** | ✅ |
| A3 adiantado = atrasado | — | −300/+300 ms: R 0,01/0,00 · −200/+200: R 0,46/0,45 · entra 190 ms antes: R 0,52 | ✅ |
| A4(b) ≤ 10 pts por 10 c | ≤ 10 | máx. 10,7 → **9,7** (Normal), 10,4 → 9,7 (Hard), 10,0 → 8,5 (Easy); 0 subidas | ✅ |
| A5 sirene / nota fixa | ≤ 15 / ≤ 25 | 9,4 / 22,0 | ✅ |
| **E1** contorno premia oscilar **em torno** do alvo | — | trinado ±100 c a 6 Hz, sem nunca cantar o alvo: 33,6 → **95,8** (pipeline) | **Alta** |
| E2 A8 saiu do scorer para o `performance.rs` | — | "só os 100 ms iniciais de cada nota chegam": 10,8 → **91,1** no scorer; vira treino pela regra de 5 % | Baixa |
| E3 perda seletiva < 5 % | — | esconder o ataque errado (3,1 % dos frames): +3,2 | Baixa (teórico) |

---

## (a) Metas do v3

- **N1 ✅** (P9a, voz realista centrada, Normal; v2 → v3):
  - vibrato ±50 c: 91,1 → 100,0;
  - vibrato ±75 c: 77,5 → 99,9;
  - vibrato ±100 c: 63,9 → 99,7;
  - com drift de até ±35 c, o pior caso é 98,6.
  - Na melodia da regressão, os números são equivalentes.
- **N2 ✅** (P14, frames ideais, canto perfeito):
  - 5, 10 e 15 % de frames perdidos ao acaso dão 100 no v3;
  - rajada de 200 ms a cada 5 s: 99,8.
  - Evidência de hardware (Agente 2): faltam de 0,16 % a 0,57 % no microfone desta máquina, cerca de 10× abaixo do limite.
- **N3 ✅** (P12, pipeline, tons ligados de 500 ms): o ritmo agora é só a rampa de tempo, com o gate ≈ 1:
  - segurar a nota anterior 150 ms: R 0,67, igual a `ramp(150)` = 0,68;
  - sustentação 0,74, sem a segunda cobrança;
  - P continua pagando pelos frames errados, como deve.
- **N4 ✅** (P13/P15d, 20 notas ligadas C4/E4):

  | | −30 ms | −25 ms | +25 ms | +30 ms | +40 ms | +50 ms |
  |---|---|---|---|---|---|---|
  | 50 ms | 100,0 | 95,8 | 95,8 | 100,0 | 51,2 | 61,9 |
  | 100 ms | 100,0 | 100,0 | 99,5 | 99,5 | 94,2 | 69,8 |

  Dentro de ±30 ms a nota curta vale cheio, como especificado. Além disso, o crédito cai de forma gradual.
- **A3 ✅** (P2/P12, pausas de 200 ms):
  - simetria: −300 ms dá R 0,01 e +300 ms dá R 0,00; −200 ms dá R 0,46 e +200 ms dá R 0,45;
  - entrar 190 ms antes dá R 0,52, perto de `ramp(190)` = 0,50;
  - as linhas "entra 250/400 ms antes" (R 0,48/0,46) equivalem na prática a entrar no início da pausa (200 ms), porque a voz segue na nota anterior até ela acabar;
  - mesma altura sustentada através de uma pausa de 150 ms: 98,1 (igual ao v2).
- **A4(b) ✅** (P10, desvio constante de 0 a 200 c em passos de 10): queda máxima de 9,7 (Normal e Hard) e 8,5 (Easy), sem nenhuma subida. Pelo pipeline: +90 c = 35,3.

## (b) Exploits novos

### E1 — O contorno de 190 ms premia oscilar em torno do alvo (Alta)

O contorno é a média dos valores a até 250 c de cada frame, numa janela de 190 ms. Uma oscilação simétrica rápida tem média exatamente no alvo, mesmo que a voz **nunca** passe por ele.

| Caso (Normal) | v2 | v3 |
|---|---|---|
| trinado quadrado −100/+100 c a **6 Hz**, nunca no alvo (pipeline, voz realista) | 33,6 | **95,8** |
| o mesmo, frames ideais | 30,2 | **83,9** (P 1,00) |
| trinado quadrado ±100 c a 3 Hz (pipeline) | 31,5 | 73,1 |
| trinado quadrado ±100 c a 1,5 Hz (frames ideais) | 30,2 | 47,3 |
| trinado quadrado ±125 c a 6 Hz | 18,7 | 59,3 |
| oscilação senoidal **±150 c** a 5,5 Hz | 33,1 | **93,5** |
| oscilação senoidal ±200 c a 5,5 Hz | 21,2 | 59,5 |
| oscilação aleatória de até ±150 c (nova a cada 80 ms) | 66,5 | 79,7 |
| alterna +1 st e +11 st a cada 100 ms (com a dobra de oitava, vira −1/+1 st) | 30,2 | **80,0** |
| comparação: **desafinação constante** +30 c / −40 c (voz realista com vibrato ±50) | 80,8 / 79,2 | 90,8 / 88,2 |

**Por que é Alta.** Não é que o trinado seja fácil de cantar (cantar o alvo é mais fácil). O problema é a **justiça do ranking**:
- uma voz que oscila ±150 c, o que já é "desafinado" de ouvido, tira 93,5;
- um trinado entre os dois semitons vizinhos tira 95,8;
- os dois passam na frente de quem canta firme a 30–40 c do alvo (88–91).

A dobra de oitava da mediana e do contorno amplia o caso: alternar +1 st e +11 st vira um trinado simétrico depois da dobra.

**Sugestão (o Agente 2 escolhe):** medir também o **espalhamento em torno do contorno**, a média de |frame − contour|:
- vibrato senoidal ±50/±75/±100 c dá cerca de 32/48/64 c;
- trinado quadrado ±100 c dá 100 c;
- senoide ±150 c dá cerca de 95 c.

Um fator de crédito que comece a reduzir acima de ~70 c, chegando a zero em ~140 c, mantém as metas de N1 e derruba os casos acima. Na regressão, acrescentar trinado ±100 c a 6 Hz e senoide ±150 c, com teto a decidir (por exemplo ≤ 50).

### E2 — A8 saiu do scorer para o `performance.rs` (Baixa)

Se só os 100 ms iniciais de cada nota chegam (P8), o scorer dá 10,8 no v2 e **91,1** no v3, porque os frames ausentes saem do denominador. No produto isso é coberto: perda > 5 % vira treino (`missing_mic_audio_over_5_percent_makes_training_with_a_reason`). Observações:
- o `missing_ratio` vem do contador do callback. Frames perdidos depois do callback, por ring buffer cheio (`capture.rs`, comentário ponytail), não entram nessa conta, e o scorer os ignoraria ❓. É raro: exige a análise travar por mais que o tamanho do ring;
- **sugestão:** um teste de regressão que cruze as duas camadas (sessão com frames faltando no meio das notas → treino).

### E3 — Perda seletiva abaixo de 5 % (Baixa, teórico)

Com ataques errados de 40 ms em todas as notas, os frames presentes dão 96,8 (v3). Se exatamente esses frames não chegam (3,1 % da música), dá 100,0. Só é explorável se alguém controlar quais frames se perdem. Perda aleatória é neutra (P14). Registro apenas.

### O que não é exploit

| Caso (Normal) | v2 | v3 |
|---|---|---|
| troca de oitava a cada 100 ms na classe certa | 100 | 100 (legítimo pela dobra) |
| trinado ±200 c | 0,1 | 0,1 |
| trinado alvo/+200 c | 32,7 | 30,1 |
| sirene | 9,7 | 9,4 |
| nota fixa | 24,4 | 22,0 |

Dentro de ±30 ms, notas curtas valem cheio para atrasado e adiantado: é o que o N4 especifica.

## (c) Leniência: desafinação constante com voz realista

P16, música de teste, drift até ±20 c + vibrato ±50 c a 5,5 Hz (v2 → v3):

| Desvio | Easy | **Normal** | Hard |
|---|---|---|---|
| 0 | 97,4 → 100,0 | 90,4 → **100,0** | 83,6 → 99,6 |
| +10 c | 96,1 → 100,0 | 89,2 → **99,7** | 82,8 → 96,7 |
| +20 c | 93,2 → 100,0 | 85,6 → **96,7** | 79,5 → 90,7 |
| +30 c | 89,5 → 98,8 | 80,8 → **90,8** | 74,8 → 82,5 |
| +40 c | 85,2 → 94,8 | 74,9 → **82,5** | 68,5 → 72,9 |
| +50 c | 80,4 → 88,4 | 68,3 → **72,9** | 60,9 → 63,2 |
| +60 c | 75,1 → 80,5 | 60,8 → 63,2 | 52,4 → 53,5 |
| +75 c | 65,5 → 67,6 | 47,7 → 48,7 | 38,8 → 39,3 |
| +90 c | 53,8 → 55,0 | 34,9 → 35,3 | 28,5 → 28,8 |
| −20 c | 95,2 → 100,0 | 88,1 → 98,5 | 81,7 → 94,8 |
| −30 c | 92,1 → 99,4 | 84,4 → 94,8 | 78,3 → 88,2 |
| −40 c | 88,3 → 97,3 | 79,2 → 88,2 | 73,3 → 78,8 |
| −50 c | 84,0 → 92,8 | 73,2 → 78,8 | 66,6 → 69,0 |

**Controle sem vibrato e sem drift** (P16b, Normal, v2 → v3):

| Desvio | 0 | +10 | +20 | +30 | +40 | +50 | +75 |
|---|---|---|---|---|---|---|---|
| Normal | 99,9 → 100,0 | 99,9 → 100,0 | 99,6 → 99,9 | 94,0 → 94,2 | 84,4 → 84,5 | 74,7 → 74,6 | 51,0 → 51,3 |

**Conclusão.**
- **O v3 não afrouxou a desafinação constante.** Sem vibrato, v2 e v3 são praticamente iguais. A subida de +30 c de 80,8 para 90,8 (e de −40 c de 79,2 para 88,2) vem só de o vibrato ter deixado de ser cobrado em cima do desvio. É o efeito pretendido do N1.
- **O Normal distingue de forma justa** "afinado" (≤ 10 c: 99,7–100), "um pouco desafinado" (30 c: 91–95) e "desafinado" (50 c: 73–79):
  - de 25 a 165 c, cai cerca de 1 ponto por cent;
  - os primeiros 20 c custam ≤ 3,3 pontos, que é a tolerância T_full = 25 c, deliberada e coerente com a percepção de amadores;
  - o Hard separa mais cedo (+20 c = 90,7; +30 c = 82,5).
- A diferença +30 c (90,8) × −30 c (94,8) é da realização do drift aleatório, não do algoritmo ❓. Sem drift, a curva só foi medida para o lado positivo.
- **A injustiça que sobra não está na curva constante. Está no E1:** oscilar ±150 c (93,5) vale mais que cantar firme a +30 c (90,8).

## Não medido ❓

- Voz real.
- Perda por ring buffer cheio (E2).
- `Scorer::skip` (seek em treino).
- `LiveFeedback`. O rótulo ao vivo agora usa a mediana do erro, e nesta rodada só o `ScoreResult` foi sondado.
