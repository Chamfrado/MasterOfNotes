# Re-revisão independente de QA: pontuação v2 (T-006-08)

Revisor: Agente 4 · 2026-11-19 · `main` @ ff25eea · `ALGORITHM_VERSION = 2`.
Escopo: `crates/scoring-engine` em main, contra a seção "Algoritmo v2" da [SPEC-006](../../../specs/SPEC-006-scoring.md) e a [revisão do v1](scoring-review.md).
Nada em `src/` foi alterado.

**Método.** Cada cenário roda **no v1 e no v2 sobre o mesmo áudio**. O v1 é o `scorer.rs` @ 86c604b, incluído como módulo dentro das sondas. As sondas são locais e não foram commitadas no crate; há uma cópia arquivada em [scoring-review-v2-probes.rs](scoring-review-v2-probes.rs), com as instruções de reprodução no cabeçalho.
Reprodução: `cargo test -p scoring-engine --test qa_probe -- --nocapture --test-threads=1` (cerca de 90 s). Resultado: 14/14 sondas executadas.
Pipeline = áudio sintético 48 kHz → `analyze_buffer` → `Scorer`. "Frames ideais" = só o Scorer. Dificuldade Normal, salvo indicação. Formato das células: `score (P/R/S) acertos`.

## Resumo

| # | Item | Situação no v2 | Severidade |
|---|---|---|---|
| A1 | Onset vazando para a nota anterior | ✅ resolvido | — |
| A2 | Voz-guia no microfone | ✅ por processo (T-004-06 exige teste de vazamento para competir); o scorer continua dando 99,7 | — |
| A3 | Adiantar-se penalizado só até certo ponto | ⚠️ **parcial**: ainda assimétrico quando a pausa antes da nota é < 300 ms; o teste A3 da regressão não cobre esse caso | Média |
| A4 | Degrau em p = 0,5 | ✅ monotônico, sem degrau; +90 c = 37,1. ⚠️ Restrição (b) ≤ 10 pts/10 c excedida por pouco: 10,7 (Normal) e 10,4 (Hard) | Baixa |
| A5 | Sirene e nota fixa | ✅ sirene 9,5–11,8 (meta ≤ 15); nota fixa 24,4 (meta ≤ 25, margem de 0,6) | — |
| A6 | Notas ligadas curtas | ✅ 50 e 100 ms ligadas, canto perfeito = 100 | — |
| A7 | Golden | ✅ `golden_results_are_exact` com 7 casos e `assert_eq!` do `ScoreResult` | — |
| A8 | Denominador = frames recebidos | ✅ resolvido (P8: 100 → 10,8), mas gerou N2 | — |
| A9 | Fixtures escondiam casos | ✅ em boa parte (a1, a3, a4, a5, a6, a9 na regressão); exceção: A3 (ver acima) | — |
| **N1** | **Vibrato largo muito penalizado** (±75 c → 77,5; ±100 c → 63,9) | novo | **Alta** |
| **N2** | **Denominador fixo + déficit do driver**: 5/10/15 % de frames perdidos → −4/−9/−12 pontos para quem canta perfeito | novo, condicional ❓ | **Alta** |
| N3 | Atraso legato punido duas vezes pelo gate: segurar a nota anterior 150 ms → 60,8 (calado no mesmo atraso → 81,5) | novo | Média |
| N4 | Notas curtas muito sensíveis à latência: 50 ms ligadas cantadas 25 ms atrasadas → 1,1 (v1 30,3) | piorou | Média |
| N5 | Curva do amador (resposta à pergunta (c)) | dado para a calibração | — |

---

## (a) A1–A9

**A1 ✅** (P3, pipeline, 500 ms ligadas, canto perfeito): todos os casos têm R = 1,00 no v2. Notas repetidas aos pares: 96,3 → **100,0**. Com consoante de 60 ms: 91,1 → 96,8. Semitons: 96,7 → **100,0**. Portamento de 60 ms entre semitons: 99,1. De 120 ms: 96,2. Saltos de oitava ligados: 94,8 → **100,0**.

**A3 ⚠️ parcial** (P2/P12, melodia da regressão com pausas de 200 ms):

| Caso | v1 | v2 |
|---|---|---|
| deslocado −300 ms (adiantado) | 72,2 (R 0,69) | 65,0 (**R 0,43**) |
| deslocado +300 ms (atrasado) | 58,3 (R 0,07) | 54,5 (**R 0,00**) |
| entra 190 ms antes e segura | 93,8 (R 0,69) | 91,8 (R 0,59) |
| entra **400 ms** antes e segura | 93,8 (R 0,69) | 90,8 (**R 0,54**) |

A janela de onset começa em `max(start − 300, fim_anterior + 25)`. Quando a pausa é menor que 300 ms, o erro de adiantamento fica limitado a `pausa − 25`. Entrar 400 ms antes vale quase o mesmo que entrar 190 ms antes, e com pausa ≤ 105 ms "encher a pausa" com a próxima nota custa **zero** de ritmo. Os frames antes da nota não entram em P nem em S.

**O teste não cobre isso.** `a3_early_and_late_cost_the_same_rhythm` usa pausas de 400 ms, com o comentário "so the ±300 ms window is not clipped". Ele prova a simetria justamente onde o corte não acontece.

**Sugestão:** se a voz já está no alvo em `onset_from`, voltar até o início real da sequência contínua (atravessando a pausa) para medir o erro. Acrescentar à regressão a melodia com pausas de 200 ms, deslocada ±300 ms.

**A4 ✅ com ressalva** (P10, frames ideais, desvio constante de 0 a 200 cents em passos de 10):
- **Monotonicidade:** zero subidas nas 3 dificuldades.
- **Maior queda entre vizinhos:** v1 40,2 / 34,5 / 44,2 (Easy/Normal/Hard) → v2 **10,0 / 10,7 / 10,4**. A restrição (b) pede ≤ 10. Excede por 0,7 em Normal (50 → 60 c: 75,9 → 65,2) e 0,4 em Hard. A SPEC registra "máx. 9,7" medido; a diferença deve ser grade ou método. Baixa.
- **Pipeline, Normal:** +40 = 85,5 · +60 = 65,2 · +90 = **37,1** (meta ≤ 40 ✅) · +95 = 32,5 · +100 = 30,1.

**A5 ✅** (P6): sirene de 1 oitava com período de 200/400/800/1600 ms = 10,3 / 9,7 / 10,9 / 9,9 (pipeline: 9,5 / 10,8). Dente de serra: 11,8 / 9,6. Nota fixa, melhor classe: 24,4. Fica 0,6 abaixo da meta de 25: qualquer ajuste futuro de tolerância pode estourá-la.

**A6 ✅** (P4/P13, canto perfeito): ligadas de 50 / 80 / 100 / 150 / 250 ms = **100** (v1: 73,7 / 80,0 / 82,6 / 89,3 / 95,3). Com 30 ms ligadas: 34,1, caso extremo.

**A8 ✅** (P8): se só os 100 ms iniciais de cada nota chegam, 100,0 → **10,8**.

**A2:** o scorer continua dando 99,7 para a voz-guia captada a −20 dB (P7). A proteção é o teste de vazamento obrigatório (T-004-06), como decidido.

## (b) Regressões e exploits novos

### N1 — Vibrato largo muito penalizado (Alta)

P9a: voz realista (formantes, jitter, shimmer), cantor **perfeitamente centrado**, Normal:

| drift / vibrato a 5,5 Hz | 0 | ±50 c | ±75 c | ±100 c |
|---|---|---|---|---|
| ±0 (música de teste) | 98,3 → 99,9 | 98,5 → **91,1** | 94,0 → **77,5** | 86,5 → **63,9** |
| ±25 (música de teste) | 98,6 → 99,8 | 97,8 → 89,8 | 93,4 → 77,0 | 84,8 → 63,1 |
| ±0 (melodia da regressão) | 99,8 → 99,7 | 99,9 → 91,4 | 95,4 → 77,4 | 87,5 → 63,3 |

**Causa.** O erro é avaliado frame a frame com T_full = 25 c. A mediana de 50 ms não remove um vibrato de 5,5 Hz (período de cerca de 180 ms). Vibrato de ±50 a ±100 c é comum em quem canta bem (pop forte, gospel, lírico), então o v2 tende a penalizar justamente os melhores cantores. A regressão só fixa ±50 c a 6 Hz (golden 91,6), no limite da meta ≥ 90.

**Sugestão:** avaliar a afinação sobre um contorno sem vibrato (média móvel de cerca de 1 período, 150–200 ms, ou o centro da oscilação), mantendo o frame a frame só para o feedback ao vivo. Acrescentar ±75 e ±100 c à regressão com metas explícitas (por exemplo ≥ 85).

### N2 — Denominador fixo + déficit de amostras do driver (Alta, condicional ❓)

P14, frames ideais, canto **perfeito**:

| Frames que não chegam | v1 | v2 |
|---|---|---|
| nenhum | 100,0 | 100,0 |
| ~5 % ao acaso | 100,0 | **96,0** |
| ~10 % ao acaso | 100,0 | **91,0** |
| ~15 % ao acaso | 100,0 | **88,1** |
| rajada de 200 ms a cada 5 s | 99,8 | 98,2 |

A própria SPEC-006 registra que, no microfone desta máquina, o WASAPI entregou **40–46 mil amostras/s com nominal de 48 mil** (déficit de 4–17 %). O resync corrige o tempo dos frames, mas não recria os que faltam. No v2, frame ausente vale 0. Se esse déficit vira frames ausentes na prática, toda nota competitiva nesse hardware cai de 4 a 12 pontos, e máquinas diferentes deixam de ser comparáveis no ranking.

❓ **Não verificado em hardware:** não sei se o déficit são lacunas reais ou artefato de contagem.

**Sugestão:**
1. Medir no aparelho: frames recebidos ÷ esperados numa execução cantada.
2. Separar "frame não vozeado" (o frame chegou com `voiced = false`, conta 0 como hoje) de "frame que não chegou" (falha de captura). O segundo fica fora do denominador, como o `skip`, e a performance vira treino se a perda passar de ~5 %.

### N3 — Atraso legato punido duas vezes pelo gate (Média)

P12, pipeline, tons ligados de 500 ms, o cantor segura a nota anterior por um tempo antes de mudar:

| Caso | v1 | v2 |
|---|---|---|
| muda no tempo | 98,7 | 100,0 |
| segura a anterior 80 ms | 86,3 | 82,2 (R 0,73, S 0,69) |
| segura a anterior 150 ms | 70,5 | **60,8** (R 0,34, S 0,38) |
| comparação: atrasado 150 ms, **calado** antes (P2) | 81,5 | 81,5 |

Os frames da nota anterior dentro da nota atual baixam a `accuracy`. O gate então multiplica R e S por `2·accuracy − 1` (≈ 0,4). O mesmo erro de tempo é cobrado em P, em R e em S. Quem canta legato e chega atrasado perde cerca de 20 pontos a mais do que quem chega igualmente atrasado, mas calado.

**Sugestão:** calcular a `accuracy` do gate só sobre os frames a partir do onset (ou de `start + 80 ms`), deixando o atraso a cargo do ritmo.

### N4 — Notas curtas e erro de latência (Média)

P13, pipeline, 20 notas ligadas alternando C4/E4, cantadas no tom certo mas atrasadas:

| Duração | no tempo | 25 ms atrasado | 50 ms atrasado |
|---|---|---|---|
| 50 ms | 100 (v1 73,7) | **1,1** (v1 30,3) | 0,0 (v1 18,6) |
| 100 ms | 100 (v1 82,6) | 72,3 (v1 63,0) | 19,5 (v1 41,7) |
| 150 ms | 100 (v1 89,3) | 85,7 (v1 73,5) | 51,6 (v1 59,9) |

Um erro de calibração de latência de 25 ms (SPEC-004 R7, ajuste manual) zera passagens rápidas de 50 ms e tira cerca de 28 pontos nas de 100 ms. O v2 ficou mais binário que o v1 nesse eixo.

**Sugestão:** documentar a precisão exigida da calibração de latência (≤ 10 ms) ou dar tolerância de tempo a notas < 120 ms (janela de pitch ampliada em ±30 ms).

### O que não é exploit (P11, frames ideais, Normal)

| Caso | v1 | v2 |
|---|---|---|
| canta só a 1ª metade de cada nota, afinada | 60,0 | 60,0 |
| só os primeiros 30 % | 44,0 | 34,2 |
| só os primeiros 150 ms | 32,0 | 16,4 |
| trinado alvo / +2 semitons a 8 Hz | 61,6 | 34,9 |
| afinado, mas picotado (100 ms sim / 100 ms não) | 58,2 | 58,2 |
| 1ª metade certa, 2ª metade uma quinta acima | 60,0 | 32,5 |

O gate contínuo não cria atalho. Cantar metade certa rende metade dos pontos, e cantar errado agora custa mais que no v1.

## (c) Curva do amador afinado (tolerâncias Normal 25/165)

P9b, música de teste (T-003-05), voz realista, **drift lento de até ±20 c + vibrato ±50 c a 5,5 Hz**, com um desvio constante (v1 → v2):

| Desvio constante | Easy | Normal | Hard |
|---|---|---|---|
| 0 | 98,3 → 97,4 | 98,0 → **90,4** | 92,0 → 83,6 |
| +10 c | 98,5 → 96,1 | 97,3 → 89,2 | 90,3 → 82,8 |
| +20 c | 98,5 → 93,2 | 95,6 → **85,6** | 85,3 → 79,5 |
| +30 c | 97,9 → 89,5 | 93,4 → **80,8** | 79,3 → 74,8 |
| −20 c | 98,0 → 95,2 | 96,7 → 88,1 | 88,2 → 81,7 |
| −40 c | 96,5 → 88,3 | 92,1 → **79,2** | 76,6 → 73,3 |

- **Drift sozinho pesa pouco:** até ±35 c custa ≤ 1 ponto (P9a, coluna "0").
- **Ataque por baixo (scoop)** de 150 c custa cerca de 3 pontos (P9c: 90,4 → 87,0); de 250 c, cerca de 7.
- **O que derruba é o vibrato** (N1) e o **desvio constante** de 20–40 c. Um amador típico (drift ±20, vibrato ±50, desvio ±20) fica entre **85 e 90** no Normal, em cima do limite de "~85" que a própria SPEC usa como gatilho de ajuste. Com 30–40 c de desvio, ou vibrato ±75, cai para **77–81**.
- ⏳ Continua valendo a ressalva da SPEC: falta confirmar com gravações reais.

## Não medido ❓

- Voz real e hardware: a confirmação de N2 depende do aparelho.
- Reverberação de sala.
- Seek em treino (`Scorer::skip`): só o caminho sem seek foi sondado.
- Feedback ao vivo (`LiveFeedback`): só o `ScoreResult` final.
