# 5ª rodada de QA (final do ciclo sintético): pontuação v5 (T-006-14)

Revisor: Agente 4 · 2026-11-19 · `main` @ 4ad7c26 · `ALGORITHM_VERSION = 5`. Nada em `src/` foi alterado.
**Método:** v4 × v5 sobre o mesmo áudio. O v4 é o `scorer.rs` @ 3439836, incluído como módulo. São 18 sondas (as 17 anteriores mais a P18, para o termo de balanço). A cópia arquivada está em [scoring-review-v5-probes.rs](scoring-review-v5-probes.rs). Os valores de referência do v3 vêm de [scoring-review-v4.md](scoring-review-v4.md). 18/18 executadas, cerca de 95 s. Normal, salvo indicação.

**Veredito: nenhum achado ALTO.** O v5 cumpre todas as metas. Os achados médios e baixos ficam registrados para a calibração com voz real.

## Metas

| Item | Meta | Medido | Situação |
|---|---|---|---|
| (a) fall −300 c / bend ±100 c / scoop lento / vibrato irregular | ≤ 2 pts abaixo do v3 (88,1 / 96,8 / 95,3 / 98,4) | **88,1 / 96,8 / 95,3 / 98,1** (v4: 78,5 / 86,6 / 90,0 / 91,4) | ✅ |
| (b) E1: trinado ±100 c a 6 Hz | ≤ 50 | pipeline 44,2 → **26,5** · frames ideais 25,0 → 9,1 | ✅ |
| E1: senoide ±150 c a 5,5 Hz | ≤ 50 | 32,4 → **28,6** | ✅ |
| E1: aleatória até ±150 c | ≤ 50 | 44,5 → **39,4** (margem de 10,6) | ✅ |
| E1: alternância +1/+11 st | ≤ 50 | 28,1 → **9,6** | ✅ |
| (c) N1 vibrato ±50/±75/±100 | ≥ 95/92/88 | 100,0 / 99,8 / 99,7 (drift ±35 + vibrato ±100: 97,5) | ✅ |
| N2 / N3 / N4 / A3 / A4(b) | metas v3 | idênticos ao v4 (A4(b) máx. 9,7; N3 72,6; N4 95,8/99,5; frames perdidos → 100) | ✅ |
| Sirene / nota fixa | ≤ 15 / ≤ 25 | 0,1 / 22,0 | ✅ |
| Curva do amador (desvio constante + vibrato ±50) | — | idêntica ao v4 (0 / +30 / −40 c → 100,0 / 90,8 / 88,1) | ✅ |

## (d) Termo de balanço e espalhamento com peso 3,5

### Falsos positivos

Pipeline, voz realista, notas de 2 s, ornamento em todas as notas:

| Canto | v4 | v5 | Severidade |
|---|---|---|---|
| vibrato lento 4 Hz ±100 c | 87,9 | 96,5 | melhorou |
| vibrato lento 3 Hz ±100 c | 18,9 | 85,3 | melhorou |
| vibrato lento 2,5 Hz ±100 c (raro) | 12,4 | 29,8 | — |
| grupeto de 300 ms (+100/0/−100/0) | 89,2 | 98,6 | melhorou |
| **volteio de 400 ms com excursão de 200 c** (+200/0/−100/0) no meio da nota | 68,7 | **39,1** | **Média** |
| **trinado ornamental de 300 ms no fim** (0/+200 c a 8 Hz) | 73,1 | **49,0** | **Média** |
| sopro SNR 5 dB no agudo (fora do escopo) | 59,8 | 51,4 | registro |

**Leitura.** Ornamentos com excursão de cerca de 200 c (riffs de R&B e gospel, trinado de fim de frase) disparam o balanço. Como a janela é de ±600 ms, o erro atinge também os frames **afinados** em volta do ornamento: um ornamento de 0,3–0,4 s zera cerca de 1,2 s da nota.

Os frames do próprio ornamento estarem fora do alvo é coerente, porque a referência não tem aquelas notas. O dano colateral não é. Ornamentos de ±100 c (grupeto) passam bem.

**Sugestão para a calibração:** limitar o balanço aos frames que estão de fato dentro da oscilação, por exemplo a janela entre a primeira e a última inversão, em vez de ±600 ms em volta de qualquer frame. Validar com gravações reais de riffs.

### Exploits e leniência (frames ideais, melodia da regressão)

| Padrão | v4 | v5 | Severidade |
|---|---|---|---|
| **triângulo ±150 c a 1 Hz** (oscilação lenta) | 11,0 | **58,5** | **Média** |
| **serra −150 → +150 c a cada 300 ms** | 31,9 | **53,4** | **Média** |
| glissando contínuo −150 → +150 c em cada nota | 22,8 | 53,1 | Baixa |
| glissando −100 → +100 c em cada nota | 52,1 | 74,5 | Baixa (coerente: um gesto único é medido só pela distância) |
| senoide lenta ±150 c a 0,4 Hz | 14,3 | 25,8 | — |
| serra −100 → +100 c a cada 600 ms | 33,4 | 17,7 | — |
| trinado quadrado ±60 c a 6 Hz (espalhamento no limite) | 99,8 | 99,8 | — (equivale a um vibrato ±60) |
| trinado quadrado 0/+120 c a 6 Hz | 65,8 | 65,8 | — (igual a constante +60 c: 66,2) |

**Leitura.** Oscilações **mais lentas** que as das metas do E1 (1 Hz, ou serra de 300 ms) passam pouco do teto de 50 (53–58,5). Elas ficam no nível de quem canta constante a +60 c (66,2) e bem abaixo de quem canta afinado. Por isso **não invertem o ranking** como o E1 original (95,8), e não classifico como Alto. Os glissandos únicos são, por definição do v5, "estilo", e pagam só pela distância ao alvo.

**Sugestão para a calibração:** acrescentar triângulo a 1 Hz e serra ±150 c ao conjunto do E1, se a voz real mostrar que isso acontece.

## Não medido ❓

- Voz real: é o próximo passo, pelo encerramento do ciclo sintético.
- `LiveFeedback`.
- O teste E2 do app foi validado na rodada 4 pelo `pnpm check` do coordenador; não executei de novo.
