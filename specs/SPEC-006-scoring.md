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

## Algoritmo v2 (decisão do coordenador, 2026-11-19, a partir da revisão de QA)

Revisão: [scoring-review.md](../docs/testing/evidence/scoring-review.md). O v1 continua válido para resultados já gravados (Lei 11). O v2 entra com `ALGORITHM_VERSION = 2`. As mudanças abaixo substituem os itens correspondentes do v1:

- **A1 + A3: janela de onset.** Busca em `[max(start − 300 ms, fim_da_nota_anterior), start + d/2]`. O erro é simétrico: `rhythm = 1` se |err| ≤ 80 ms, `0` se |err| ≥ 300 ms, linear entre. Nota colada na anterior (gap 0) começa a busca em `start`, então nota repetida ligada cantada no tempo tem ritmo 1.
- **A4: sem degrau.** Sustain e onset deixam de usar o corte p ≥ 0,5. Restrições obrigatórias, a formulação é do Agente 2:
  (a) o score é monotonicamente não crescente em |e| para desafinação constante;
  (b) nenhum salto > 5 pontos entre desafinações constantes vizinhas a 10 cents;
  (c) +90 cents constante (quase a nota vizinha) ≤ 40 no Normal.
- **A5: ritmo só em notas acertadas.** `rhythm_i = 0` se `pitch_i < 0,5`. Meta: sirene de 1 oitava e nota fixa ≤ 15.
- **A6: zona de transição.** Quando a nota anterior termina a < 30 ms do início desta, os primeiros `min(25 ms, d/4)` são excluídos do pitch e do sustain. O mesmo vale para o final, em relação à próxima nota. Meta: notas ligadas de 100 ms, canto perfeito ≥ 92; de 50 ms ≥ 85.
- **A8: denominador fixo.** O denominador de `pitch_i`/`sustain_i` é o número de frames **esperados** da nota (d/10 ms, menos as zonas de transição). Frame ausente conta 0. Trechos pulados por seek ficam fora (`Scorer::skip`).
- **A7: golden.** A regressão ganha `assert_eq!` do `ScoreResult` completo (valores exatos) para cada fixture. Qualquer mudança de resultado quebra o teste e exige bump de versão e atualização desta spec.
- **A9: fixtures realistas.** Notas repetidas ligadas, legato por semitom, notas curtas (30/50/100 ms), adiantado e atrasado ±300 ms, sirene, nota fixa e voz realista (formantes, jitter, shimmer, sopro), incorporadas de `qa_probe.rs` (Agente 4).
- **A2: voz-guia no microfone.** Não dá para resolver só no scorer. Decisão: a **checagem de vazamento** (SPEC-004 R9) passa a ser **obrigatória para modo competitivo**, por dispositivo e volume. O resultado fica salvo nas settings (`leakCheck: { deviceId, videoVolume, passed, atMs }`). Sem checagem aprovada para o dispositivo e o volume atuais, a performance é treino, e a UI explica e sugere fones. Tarefa T-004-06. Até ela existir, é um risco aceito e registrado no roadmap.

Critérios v1 que continuam valendo: toda a tabela acima, com as metas acrescentadas pelos itens acima.

## PLAN
`scoring-engine` puro. `Scorer` incremental; `finish()` consolida. 5 min ≈ 30k frames em memória (ok). `performance.rs` (src-tauri) faz âncora/sync, aplica latência e decide `competitive`.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-006-01 | `Scorer` v1 conforme fórmula + testes unitários | Agente 2 | done |
| T-006-02 | Regressão com gerador de fixtures sintéticas (`tests/regression.rs`) cobrindo a tabela | Agente 2 | done |
| T-006-03 | `performance.rs`: âncora, resync, pausa, seek→treino, latência | Agente 2 | review |
| T-006-04 | Relatório de precisão em `docs/testing/scoring-accuracy.md` | Agente 2 | done |
| T-006-05 | Persistência da performance + recordes | Agente 1 | review |
| T-006-06 | Algoritmo v2 (A1, A3–A9) + golden + fixtures realistas, `ALGORITHM_VERSION = 2` | Agente 2 | todo |
| T-004-06 | Checagem de vazamento obrigatória para modo competitivo (A2): medição no audio-engine/performance, settings `leakCheck`, tela no assistente de microfone | Agente 2 + Agente 1 + Agente 3 | todo |

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
- Item 9: `exploratory_metrics(&[VoiceFrame]) -> ExploratoryMetrics { voiced_ratio, pitch_stability }` (contrato §2.2). Corridas vozeadas são separadas por frame não vozeado ou por lacuna > 50 ms, e só contam as que têm ≥ 200 ms. Sem corrida qualificada, a estabilidade é 0. Testes: tom estável > 0,999; glissando de 1 oitava < 0,1; vibrato ±30 cents ≈ 0,79; silêncio → 0.
- T-006-03 `review` (Agente 2, 2026-11-19): `apps/desktop/src-tauri/src/performance.rs` conforme contrato §2.4.
  - A âncora usa `Capture::captured_samples()` (contador do callback).
  - `played_ms` é medido no relógio do **vídeo** (Σ avanço entre syncs tocando; a cauda depois do último sync é estimada pelo microfone). No microfone desta máquina, o WASAPI entregou 40–46 mil amostras/s com nominal de 48 mil, então o relógio de amostras perde tempo. O resync de 40 ms corrige o mapeamento dos frames.
  - Ticks saem a cada 3 frames.
  - Seek → treino, com `Scorer::skip` nas notas puladas.
  - `leakSuspected` é só indicativo.
  - Testes: 9 unitários da sessão (sem hardware) + 1 `#[ignore]` com microfone real (5/5 verdes). Rodados num crate-harness fora do repositório que inclui o arquivo via `#[path]` (regra de disco: Agente 2 não compila o app). A compilação dentro do app fica com o Agente 1 ao registrar `mod performance;`.
- Tarefas review → done (Agente 0, 2026-11-19): integrado em main @ 566a64e; fmt, clippy --workspace -D warnings e cargo test --workspace verdes (80 testes).
- T-006-05 `review` (Agente 1, 2026-11-19): `src-tauri/src/live.rs` registra `performance.rs` e expõe `start_performance` (para o medidor, substitui uma execução esquecida, valida música, participante e rodada (sessão ativa, rodada não concluída, música/participante batem), monta `PerformanceConfig` com a referência ativa e as settings, explica em `reason` por que não é competitivo), `sync_performance`, `finish_performance` e `abort_performance`. Persistência: com referência sempre grava (competitiva ou treino); sem referência não grava nada e devolve as métricas exploratórias. `competitive` final = `summary.competitive && played_ms ≥ 50% de duration_ms` (duração desconhecida = treino). Rodada vira `completed` só com performance competitiva e sessão ativa. `personalBest`/`songRecord` na CompatKey da execução (referência e revisão do início). `audio://error`: watchdog a cada 250 ms no medidor e na performance; ao falhar, o stream é parado (a performance é descartada) e o evento é emitido. Testes ✅ `live::tests::{prepare_reports_competitiveness, competitive_needs_half_the_song, unknown_duration_is_training_and_no_reference_stores_nothing, round_completes_and_records_per_participant}`; os 9 testes de `performance.rs` passam dentro do app; `performance::tests::live_run_on_the_default_microphone` (`--ignored`) passou no microfone padrão desta máquina.
