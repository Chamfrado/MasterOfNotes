# 4ª rodada de QA (curta): pontuação v4 (T-006-12)

Revisor: Agente 4 · 2026-11-19 · `main` @ 3439836 · `ALGORITHM_VERSION = 4`. Nada em `src/` foi alterado.
**Método:** v3 × v4 sobre o mesmo áudio. O v3 é o `scorer.rs` @ 7caa1c9, incluído como módulo. São 17 sondas (as 16 anteriores mais a P17, que procura falsos positivos do termo de movimento). A cópia arquivada está em [scoring-review-v4-probes.rs](scoring-review-v4-probes.rs). 17/17 executadas, cerca de 90 s. Normal, salvo indicação.

## Resumo

| Item | Meta | v3 → v4 | Situação |
|---|---|---|---|
| (a) E1: trinado ±100 c a 6 Hz | ≤ 50 | pipeline 95,8 → **44,2** · frames ideais 83,9 → 25,0 | ✅ |
| E1: senoide ±150 c a 5,5 Hz | ≤ 50 | 93,5 → **32,4** | ✅ |
| E1: aleatória até ±150 c | ≤ 50 | 79,7 → **44,5** | ✅ (margem de 5,5) |
| E1: alternância +1/+11 st | ≤ 50 | 80,0 → **28,1** | ✅ |
| (b) E2: "só os 100 ms iniciais chegam" → treino | treino | o scorer continua dando 91,1. O teste `ring_drops_count_as_missing_audio_across_scorer_and_session` cobre a sessão: missing > 0,8 ⇒ treino | ✅ por leitura; não executei ❓ (compila o app) |
| (c) N1 vibrato ±50/±75/±100 | ≥ 95/92/88 | 100,0 / 99,8 / 99,7 (drift ±35 + vibrato ±100: 98,0) | ✅ |
| N2, N3, N4, A3, A4(b), sirene, nota fixa | metas v3 | idênticos ao v3. A4(b) máx. 9,7; sirene 0,5; nota fixa 22,0 | ✅ |
| Curva do amador (desvio constante + vibrato ±50) | — | igual ao v3, com diferença ≤ 0,2 | ✅ |
| (d) **Falsos positivos do termo de movimento** | — | portamento, fall, bend e vibrato irregular: **−5 a −10** | **Média** |
| Voz com muito ruído (sopro a SNR 5 dB) no agudo | — | 73,7 → **59,8** | Média |
| Ligadas de 100 ms cantadas 50 ms atrasadas | — | 69,8 → 56,3 | Baixa |
| Exploits novos | — | nenhum encontrado | ✅ |

## (d) Falsos positivos em canto legítimo (P17)

Pipeline, voz realista, notas de 2 s com pausas de 300 ms. O ornamento é aplicado **em todas as notas**; numa música real, o custo é proporcional à fração de notas com ornamento.

| Canto legítimo | v3 | v4 | Δ |
|---|---|---|---|
| referência: centrado / vibrato ±60 c | 100,0 / 100,0 | 100,0 / 100,0 | 0 |
| scoop lento: −100 c → 0 em 400 ms | 95,3 | **90,0** | −5,3 |
| queda no fim (fall): −300 c nos últimos 250 ms | 88,1 | **78,5** | −9,6 |
| bend no meio da nota: +100 c e volta, em 300 ms | 96,8 | **86,6** | −10,2 |
| vibrato irregular: taxa 4–7 Hz, extensão 30–90 c | 98,4 | **91,4** | −7,0 |
| vibrato que entra na 2ª metade (±75 c) | 100,0 | 100,0 | 0 |
| afinação que sobe ao longo da nota: −30 → +30 c / −50 → +50 c | 99,7 / 94,2 | 99,7 / 94,2 | 0 |
| jitter de 1 %, 2 % e 3 % | 100 | 100 | 0 |
| scoop rápido de 150 c em 100 ms (P9c) | 98,3 | 97,6 | −0,7 |

**Leitura.** O termo `2·max(0, movimento − 20 c)` não distingue **oscilação** (centro que vai e volta: trinado, voz instável) de **gesto expressivo monotônico** (portamento, fall, bend). Um centro que anda 100 c em 200 ms gera 160 c de erro nesses frames. Movimento lento (crescendo de afinação) e jitter passam ilesos. Ornamentos comuns em pop e blues custam 5–10 pontos quando aparecem em todas as notas.

**Sugestão (o Agente 2 escolhe):** cobrar o movimento só quando o centro **inverte de direção** dentro de ~0,5–1 s, isto é, oscilação. Outra opção é medir a variância do centro numa janela maior, em vez da diferença ±100 ms. Atenção à margem do E1: a oscilação aleatória ±150 c está em 44,5, perto do teto de 50. Na regressão, acrescentar fall, bend e scoop lento com piso (por exemplo ≥ 90).

**Ruído forte no agudo** (P5, sopro a SNR 5 dB, C5–A5): 73,7 → **59,8** (Hard 71,4 → 57,5). Os erros da análise (68 % dos frames a ≤ 50 c) agora também contam como espalhamento e movimento. É um caso extremo, de microfone ruim ou sala barulhenta com soprano, mas é um falso positivo do mesmo termo. Com SNR ≥ 10 dB, nada muda.

**Baixa** (P13): ligadas de 100 ms cantadas 50 ms atrasadas (fora da tolerância do N4, que é ±30 ms) caem de 69,8 para 56,3. Dentro da tolerância, nada muda.

## Exploits novos

Nenhum encontrado.
- Os termos de espalhamento e movimento só **somam** erro, então não podem aumentar o crédito de nenhum frame.
- O novo corte do contorno no onset da próxima nota não cria vantagem. Sair 80 ms antes: 94,2 → 91,0. Sair 150 ms antes: 75,7 → 73,3.
- O recuo do onset de até 1 s não alterou nenhuma sonda de tempo (P2, P12, P15d iguais ao v3).
- Perda seletiva abaixo de 5 % (E3): igual ao v3 (+3,2), risco já aceito.

## Não medido ❓

- Execução do teste E2 de `performance.rs` (compila o app; fica com o `pnpm check` do coordenador).
- Voz real.
- `LiveFeedback`.
