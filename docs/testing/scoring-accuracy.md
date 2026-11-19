# Precisão da análise e da pontuação (algoritmo v1)

Dono: Agente 2. Specs: [SPEC-004](../../specs/SPEC-004-microphone.md), [SPEC-006](../../specs/SPEC-006-scoring.md).
Medido em 2026-11-19 · `ALGORITHM_VERSION = 1` · Intel Core i5-11400H, 31,8 GB RAM, Windows 11, rustc 1.99.1.

Todos os números abaixo vêm de testes executados e reproduzíveis (áudio gerado em código, sem arquivos binários).
Nenhum foi medido com microfone físico: esse teste está pendente (⏳, ver o fim deste documento).

## Pontuação: tabela de aceite da SPEC-006 (pipeline completo)

Caminho completo: áudio sintético a 48 kHz → `audio_engine::analyze_buffer` → `Scorer` (Normal).
Tempo de cada frame = centro da janela de análise.
Música de teste: 10 notas de 1000 ms com pausas de 200 ms, começando em 500 ms, melodia C4 D4 E4 F4 G4 A4 G4 F4 E4 D4.
Voz em amplitude 0,2 (≈ −17 dBFS RMS), gate padrão (piso −60 dBFS + 10 dB).

Reproduzir: `cargo test -p scoring-engine --test regression -- --nocapture --test-threads=1`

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

## Pendente ⏳

- Validação com microfone físico e voz real (T-004-02/03): exige hardware e será registrada em `docs/testing/evidence/` quando feita.
- Calibração de latência real (SPEC-004 R7) e vazamento da música para o microfone (R9).
