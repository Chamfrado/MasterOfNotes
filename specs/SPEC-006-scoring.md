# SPEC-006 — Pontuação vocal (algoritmo v1)

Contrato: [contracts.md §2.2](../docs/architecture/contracts.md#22-scoring-engine-agente-2).

## SPEC — definição matemática

Baseline do coordenador. Ajustes são permitidos ao Agente 2 **com evidência das fixtures**, atualização desta seção e bump de `ALGORITHM_VERSION` se algum resultado mudar.

Entradas: notas da referência (offset aplicado) e `VoiceFrame`s a cada 10 ms em tempo de música.

1. **Erro em cents**: `e = 1200·log2(f0 / f_ref)`. **Oitava**: `e ← e − 1200·round(e/1200)` (faixa (−600, 600]). Cantar uma oitava acima/abaixo é aceito (vozes diferentes), sem bônus nem penalidade.
2. **Suavização**: f0 passa por mediana móvel de 5 frames (50 ms) antes de calcular `e` (reduz erros pontuais e vibrato).
3. **Pitch por frame** (frame `voiced` dentro de nota): `p = 1` se |e| ≤ T_full; `0` se |e| ≥ T_zero; linear entre. Easy (75, 200), Normal (50, 150), Hard (30, 100) cents.
4. **Frame não-voiced** em nota: p = 0. Frames fora de notas (instrumental/pausas previstas) são ignorados.
5. **Por nota i** (duração d_i):
   - `pitch_i` = média de p sobre **todos** os frames da nota.
   - `sustain_i` = fração dos frames da nota com p ≥ 0,5.
   - `rhythm_i`: onset = primeiro frame com p ≥ 0,5 em [start − 150 ms, start + d_i/2]; `err = |onset − start|`; 1 se err ≤ 80 ms, 0 se err ≥ 300 ms, linear entre; sem onset → 0.
   - `hit_i` = pitch_i ≥ 0,5.
6. **Agregação** ponderada por duração sobre **todas** as notas (nota não cantada contribui 0 — impede nota máxima com poucas notas):
   `P = Σd·pitch / Σd`, `R = Σd·rhythm / Σd`, `S = Σd·sustain / Σd`.
   **score = 100 · (0,65·P + 0,20·R + 0,15·S)**, arredondado a 0,1. `coverage = notes_hit / notes_total`.
7. **Feedback ao vivo** (p médio dos últimos 200 ms em nota): Excellent ≥ 0,9; VeryGood ≥ 0,75; Good ≥ 0,5; NeedsWork < 0,5; NotDetected se os frames recentes são não-voiced. `running_score` = score parcial considerando só notas já terminadas.
8. **Determinismo**: mesma sequência de frames ⇒ mesmo resultado. Sem aleatoriedade, sem paralelismo dependente de ordem.
9. **Sem referência**: nenhum score numérico; só métricas exploratórias (estabilidade de afinação, % de voz detectada) marcadas como não competitivas.
10. **Volume** não aparece em nenhuma fórmula (só via gate do VAD).
11. **Transposição/mudança de tom**: a referência define o tom; cantar em outra tonalidade (exceto oitava) é desafinação. Ornamentos curtos (< 50 ms fora da nota) são absorvidos pela mediana.

Critérios de aceite (fixtures sintéticas, dificuldade Normal):

| Caso | Esperado |
|---|---|
| Perfeito (senoide na nota, onset exato) | score ≥ 98 |
| Perfeito uma oitava abaixo | score ≥ 98 |
| +100 cents constante | P ∈ [0,45; 0,55] |
| +200 cents constante | P = 0 e score ≤ 25 |
| Silêncio | score = 0 |
| Ruído branco | score ≤ 5 |
| Atraso de 200 ms em todas as notas | R ∈ [0,3; 0,6] e P ≥ 0,7 |
| Só 1 de 10 notas iguais cantada | score ≤ 15 |
| Perfeito com volume 4× | score idêntico ao perfeito |
| Vibrato ±50 cents a 6 Hz | score ≥ 90 |
| Voz + acompanhamento a −12 dB | score ≥ 85 |
| Perfeito com glissando de 80 ms entre notas | score ≥ 92 |

## PLAN
`scoring-engine` puro. `Scorer` incremental; `finish()` consolida. 5 min ≈ 30k frames em memória (ok). `performance.rs` (src-tauri) faz âncora/sync, aplica latência e decide `competitive`.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-006-01 | `Scorer` v1 conforme fórmula + testes unitários | Agente 2 | review |
| T-006-02 | Regressão com gerador de fixtures sintéticas (`tests/regression.rs`) cobrindo a tabela | Agente 2 | review |
| T-006-03 | `performance.rs`: âncora, resync, pausa, seek→treino, latência | Agente 2 | todo |
| T-006-04 | Relatório de precisão em `docs/testing/scoring-accuracy.md` | Agente 2 | review |
| T-006-05 | Persistência da performance + recordes | Agente 1 | todo |

## TEST
`scoring-engine/tests/regression.rs` cobre cada linha da tabela. Pipeline completo (áudio sintético → `analyze_buffer` → `Scorer`) para pelo menos perfeito, desafinado, silêncio, atraso.

## STATUS
- T-006-01/02/04 `review` (Agente 2, 2026-11-19): `Scorer` v1 (`ALGORITHM_VERSION = 1`). Todas as 12 linhas da tabela passam pelo pipeline completo (áudio → `analyze_buffer` → `Scorer`). Números em [scoring-accuracy.md](../docs/testing/scoring-accuracy.md). Nenhum parâmetro da baseline foi alterado.
- Interpretações (sem mudar a fórmula):
  - a mediana é centrada (±2 frames), só sobre frames vozeados, em unidades MIDI. Por isso o feedback ao vivo chega 2 frames (20 ms) atrasado;
  - `pitch_i` e `sustain_i` são a média sobre os frames recebidos dentro da nota. Nota sem frames vale 0;
  - frames antes do início da nota (até −150 ms) contam só para o onset;
  - `NotDetected` aparece quando menos da metade dos frames dos últimos 200 ms em nota é vozeada;
  - `pitch`, `rhythm`, `sustain` e `coverage` em `ScoreResult` vão de 0 a 1, e `score` vai de 0 a 100.
- Item 9 (métricas exploratórias sem referência): não implementado. Não está no contrato §2.2.
- T-006-03 (`performance.rs`): onda 2.
