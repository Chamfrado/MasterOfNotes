# Revisão independente de QA: pontuação v1

Revisor: Agente 4 · 2026-11-19 · `main` @ 86c604b · `ALGORITHM_VERSION = 1`.
Escopo: `crates/audio-engine`, `crates/scoring-engine`, [scoring-accuracy.md](../scoring-accuracy.md), contra [SPEC-004](../../../specs/SPEC-004-microphone.md) e [SPEC-006](../../../specs/SPEC-006-scoring.md).
Pergunta: onde a nota pode ser **injusta** ou **explorável**, e onde os testes provam menos do que afirmam.

**Como reproduzir.** As sondas estão em `crates/scoring-engine/tests/qa_probe.rs`, na branch `agent-4/work`. Não serão integradas sem acordo do Agente 2.
Comando: `cargo test -p scoring-engine --test qa_probe -- --nocapture --test-threads=1` (cerca de 30 s).
Todos os números abaixo vieram dessa execução. Pipeline = áudio sintético 48 kHz → `analyze_buffer` → `Scorer`. "Só Scorer" = frames ideais a cada 10 ms, para isolar a regra da análise. Dificuldade Normal, salvo indicação. Nenhum código em `src/` foi alterado.

## Resumo

| # | Achado | Severidade | Tipo |
|---|---|---|---|
| A1 | A janela de onset (150 ms antes) enxerga a nota anterior: notas repetidas, ligadas ou rápidas perdem ritmo com canto perfeito | **Alta** | injusto |
| A2 | Voz-guia do vídeo captada pelo microfone pontua 99,8 com o usuário calado | **Alta** | explorável |
| A3 | Adiantar-se tem penalidade de ritmo limitada a 150 ms; entrar cedo e segurar a nota rende 93,8 | Média | explorável |
| A4 | Degrau no limite p = 0,5: +99 cents dá 67,6 e +100 dá 32,4; cantar quase um semitom errado ainda rende cerca de 70 | Média | injusto / leniente |
| A5 | Sirene ou nota fixa sem cantar a melodia dá cerca de 31, mais que uma tentativa real fora do tom (0) | Média | explorável |
| A6 | Notas ligadas curtas: canto perfeito de 100 ms dá 82,6 e de 50 ms dá 73,7 | Média | injusto |
| A7 | A regressão usa limiares, não valores exatos: mudança de resultado não força bump de `ALGORITHM_VERSION` | Média | teste prova menos |
| A8 | Média sobre frames **recebidos**: se só 10% dos frames de cada nota chegam, a nota é 100 | Baixa | robustez |
| A9 | As fixtures da regressão escondem A1, A3 e A6 (só notas de 1000 ms com pausas de 200 ms ≥ 150 ms) | Média | teste prova menos |

O que se sustentou está na seção "Pontos fortes confirmados". A voz realista foi robusta.

---

## A1 — Onset "vaza" para a nota anterior (Alta)

**Causa.** O onset é o primeiro frame com p ≥ 0,5 em `[start − 150 ms, start + d/2]` (`scorer.rs:229-237`). Se a nota anterior, ou a de duas notas antes, tem a mesma classe de altura (ou está a ±100 cents), o cantor que a sustenta perfeitamente já satisfaz p ≥ 0,5 no início da janela. O onset cai em `start − 150` e o ritmo vira 0,688.

**Medido (P3, pipeline, notas ligadas de 500 ms, canto perfeito):**

| Melodia | Score | R |
|---|---|---|
| Tons distintos (2 semitons), contínuo | 98,7 | 1,000 |
| Notas repetidas aos pares, contínuo | 96,3 | **0,844** |
| Notas repetidas aos pares, com consoante (60 ms sem voz antes de cada nota) | 91,1 | **0,844** |
| Semitons alternados, contínuo | 96,7 | **0,844** |

- A consoante não resolve: os frames em `[start − 150, start − 60)` ainda são da nota anterior.
- Pior caso, com todas as notas repetidas: R ≈ 0,72, ou seja, até cerca de 6 pontos a menos para um cantor perfeito. Notas repetidas em sílabas seguidas são muito comuns em música popular.
- **Semitons (fragilidade numérica):** a 100 cents, p = 0,5 exatamente. Só metade das transições (uma das direções) disparou o onset antecipado. Quem decide é o arredondamento de float da análise, não o cantor.
- **Aparece também em P4:** com notas de 50 a 100 ms alternando C4 e E4, a nota de duas posições atrás cai dentro dos 150 ms. Com 50 ms e pausas iguais (canto perfeito), o score é 94,4 e R = 0,719.

**Sugestão (o Agente 2 escolhe):** o onset deve ser uma **transição para a nota**. Aceitar o onset só em um frame com p ≥ 0,5 cujo frame anterior tenha p < 0,5 ou seja não vozeado. Se o cantor já está na altura continuamente desde dentro de uma nota anterior com o mesmo alvo, considerar erro = 0 (ligadura legítima). Fixture nova: melodia ligada com notas repetidas, esperando R ≥ 0,95.

## A2 — Voz-guia vazando no microfone (Alta)

**Medido (P7, pipeline, usuário em silêncio, apenas a voz-guia com formantes chegando ao microfone, gate padrão de −50 dBFS):**

| Voz-guia no microfone | Score |
|---|---|
| −20 dB em relação à voz de teste (≈ −37 dBFS RMS) | **99,8** |
| −30 dB | 58,9 |
| −35 dB ou menos | 0,0 |

Muitos vídeos de karaokê trazem voz-guia. Com caixas de som e o microfone do notebook, basta ficar calado para chegar ao topo do ranking. O aviso de vazamento da SPEC-004 R9 só **avisa** e ainda não está implementado.
**Sugestão:** vazamento detectado (R9) deve tornar a performance **não competitiva**, entrando como condição em SPEC-007 R2 (implementação em `performance.rs`, T-006-03). Medir o vazamento continuamente durante as pausas previstas da referência, e não só na calibração. Registrar o percentual em `capture_json`.

## A3 — Cantar adiantado é penalizado só até 150 ms (Média)

A janela de onset começa em `start − 150`. Quem entra antes disso ganha onset = `start − 150` e erro = 150, nunca mais.

**Medido (P2, pipeline):**

| Caso | Score | P | R |
|---|---|---|---|
| Adiantado 300 ms | 72,2 | 0,730 | **0,688** |
| Atrasado 300 ms | 58,3 | 0,712 | **0,069** |
| Entra 190 ms antes e segura até o fim da nota | **93,8** | 1,000 | 0,688 |
| Entra 150 ms atrasado e termina no fim | 81,5 | 0,850 | 0,676 |

A estratégia "entrar sempre cedo e segurar" fica 12 pontos acima do mesmo erro de tempo cometido no sentido do atraso. Os frames antes da nota são ignorados em P e S, então adiantar-se não custa nada além do ritmo limitado.
**Sugestão:** se o frame em `start − 150` já tem p ≥ 0,5, voltar para trás até o início real da sequência contínua e usar esse instante como onset. Isso resolve junto com A1.

## A4 — Degrau no limite p = 0,5 e leniência a quase um semitom (Média)

Sustain e onset usam `p ≥ 0,5`, que no Normal equivale a |e| ≤ 100 cents. Juntos, eles pesam 0,35 e mudam de valor no mesmo ponto.

**Medido (P1, pipeline, desvio constante):**

| Desvio | +75 | +90 | +95 | +99 | **+100** | +101 | +125 |
|---|---|---|---|---|---|---|---|
| Score | 83,7 | 73,9 | 70,6 | 67,6 | **32,4** | 31,8 | 16,2 |

- **Degrau de 35 pontos em 1 cent.** No limite exato, o lado do degrau depende do erro subcent da análise (a regressão reconhece isso e só verifica P).
- Cantar **90 cents fora** (praticamente a nota vizinha) rende 73,9 e 10/10 notas acertadas. Isso contradiz o item 11 da SPEC-006 ("outra tonalidade é desafinação").
- Os degraus equivalentes: Easy a 137,5 cents, Hard a 65 cents.

**Sugestão:** exigir |e| ≤ T_full (p = 1) para sustain e onset, em vez de p ≥ 0,5. O degrau passa para 50 cents no Normal, onde o cantor já está claramente afinado, e a leniência ao semitom acaba. A alternativa é registrar o degrau na SPEC como intencional. Qualquer mudança exige bump de `ALGORITHM_VERSION`.

## A5 — Sirene e nota fixa pontuam sem cantar a melodia (Média)

**Medido (P6, Normal; melodia da regressão):**

| Estratégia | Score | P | R |
|---|---|---|---|
| Sirene de 1 oitava, período 400 ms (só Scorer) | **31,0** | 0,156 | 0,923 |
| Sirene de 1 oitava, período 400 ms (pipeline, áudio) | 29,9 | 0,156 | 0,871 |
| Dente de serra, período 100 ms | 30,0 | 0,189 | 0,741 |
| Nota fixa contínua (melhor classe de altura) | 31,0 | 0,300 | 0,273 |
| Tentativa real cantada toda +200 cents | **0,0** | 0 | 0 |

O ritmo é barato: basta **um** frame com p ≥ 0,5 perto do início, e a sirene passa por todas as classes de altura. Não ameaça o topo do ranking, mas inverte a ordem entre cantores fracos (num Duelo, quem faz sirene vence quem tentou cantar).
**Sugestão:** contar o ritmo só em notas acertadas (`rhythm_i = 0` se não houver hit), ou exigir um onset sustentado (≥ 3 frames consecutivos com p ≥ 0,5). Pelos números de P6, a sirene cairia para cerca de 13. Os casos da tabela de aceite não mudam, porque todos têm 10/10 hits ou 0.

## A6 — Notas ligadas curtas perdem nota com canto perfeito (Média)

**Medido (P4, pipeline, 20 notas alternando C4 e E4, canto perfeito):**

| Duração | 30 ms | 50 ms | 80 ms | 100 ms | 150 ms | 250 ms |
|---|---|---|---|---|---|---|
| Ligadas | 32,3 | 73,7 | 80,0 | 82,6 | 89,3 | 95,3 |
| Com pausa igual à duração | 96,8 | 94,4 | 100 | 100 | 100 | 100 |

**Causa.** A janela de análise de 42,7 ms, com o frame datado no centro, mais a mediana de ±2 frames, borram cerca de 40 a 50 ms em cada fronteira de nota ligada. Somam-se os efeitos de A1 no ritmo.
**Sugestão:** zona de transição. Ignorar em P e S os frames a menos de `min(25 ms, d/4)` das bordas de notas **ligadas**. Os casos com pausa provam que o problema está só na fronteira.

## A7 — A regressão não detecta mudança de resultado (Média)

`regression.rs` só verifica limiares (`score ≥ 98`, `P ∈ [0,45; 0,55]`). Uma mudança de algoritmo que leve o perfeito de 100,0 para 98,5, ou o vibrato de 100 para 91, passa verde sem bump de `ALGORITHM_VERSION`, o que contradiz a SPEC-006 e as leis 11 e 12. O teste `scoring_is_deterministic` compara duas execuções no mesmo processo, então só prova a ausência de aleatoriedade.
**Sugestão:** um teste "golden" com o `ScoreResult` exato (`assert_eq!`) de 4 ou 5 fixtures (perfeito, +98 cents, atraso de 200 ms, vibrato, acompanhamento), mais a constante `ALGORITHM_VERSION` na mesma asserção. A mensagem de erro deve mandar fazer o bump.

## A8 — Média sobre frames recebidos (Baixa)

Recebendo só os 100 ms iniciais de cada nota (por exemplo, captura travada), o score é **100,0** (P8). O `Scorer` não sabe quantos frames deveriam ter chegado.
**Sugestão:** usar como denominador os frames esperados (`d / 10 ms`), ou fazer `performance.rs` marcar como não competitivo quando a cobertura de frames for menor que 95%.

## A9 — As fixtures escondem os casos difíceis (Média)

A música da regressão tem só notas de 1000 ms separadas por pausas de 200 ms. A pausa é maior que a janela de onset de 150 ms, o que esconde A1 e A3. Não há notas curtas (A6), notas repetidas nem frase ligada.
**Sugestão:** acrescentar à regressão uma segunda música ligada, com notas repetidas, notas de 100 a 250 ms e saltos de semitom, com expectativas de canto perfeito ≥ 95.

---

## Pontos fortes confirmados

- **Voz realista** (P5, pipeline): vogal /a/ com formantes (F1 700, F2 1220, F3 2600 Hz), harmônicos até 4 kHz, jitter de 1%, shimmer de 10% e drift de 15 cents, nos registros grave (C3–A3), médio e agudo (C5–A5).
  - Score de 99,8 a 100 em Normal e Hard, com 100% dos frames vozeados e corretos.
  - Com sopro de SNR 10 dB, ainda ≥ 99,8.
  - Só com sopro extremo (SNR 5 dB) cai: 98,7 no grave, 93,0 no médio e 74,9 no agudo (68% dos frames corretos).
  - A preocupação de que "a senoide é fácil demais" **não se confirmou** para jitter, formantes e ruído moderado.
- Invariância a volume, oitava, silêncio, ruído e vibrato de ±50 cents: confirmados pelos testes do Agente 2 e coerentes com P1 e P2.
- Determinismo: sem aleatoriedade nem paralelismo. `diff` do YIN com ordem fixa de faixas; Rust não reassocia floats.
- Cantar atrasado (P2) é simétrico e gradual até 300 ms, conforme a SPEC.

## Não medido ❓

- **Reverberação de sala:** a cauda da nota anterior entra na seguinte e deve piorar A1 e A6. Não sintetizei convolução.
- Voz real e microfone físico: dependem de hardware (T-004-02/03).
- Determinismo entre plataformas: `log2` e `powf` dependem da libm. Só importa no limite de A4 e só se houver alvo fora do Windows.
- Frames fora de ordem ou duplicados no `push`: dependem de `performance.rs` (T-006-03), ainda inexistente.
- `qa_probe.rs` leva cerca de 30 s. Se for integrado, sugiro marcar com `#[ignore]`.
