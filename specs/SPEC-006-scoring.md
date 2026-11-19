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
  (b) nenhum salto > 10 pontos entre desafinações constantes vizinhas a 10 cents. Revisado em 2026-11-19: a meta original de 5 era matematicamente incompatível com (c) e com perfeito ≈ 100, que exigem ≥ 6,7 por passo; medido no v2: máx. 9,7;
  (c) +90 cents constante (quase a nota vizinha) ≤ 40 no Normal.
- **A5: ritmo só em notas acertadas.** `rhythm_i = 0` se `pitch_i < 0,5`. Meta: sirene de 1 oitava ≤ 15; nota fixa ≤ 25. Revisado: quem sustenta uma classe canta de fato, afinadas, as notas daquela classe, e o crédito é legítimo, coerente com "1 de 10 notas = 10". **Formulação aceita no v2:** em vez do corte `pitch_i < 0,5 → 0`, ritmo e sustain são multiplicados pelo fator contínuo `clamp(2·acerto − 1, 0, 1) · min(1, 2·pitch_i)`, que é compatível com A4 (sem degrau).
- **A6: zona de transição.** Quando a nota anterior termina a < 30 ms do início desta, os primeiros `min(25 ms, d/4)` são excluídos do pitch e do sustain. O mesmo vale para o final, em relação à próxima nota. Meta: notas ligadas de 100 ms, canto perfeito ≥ 92; de 50 ms ≥ 85.
- **A8: denominador fixo.** O denominador de `pitch_i`/`sustain_i` é o número de frames **esperados** da nota (d/10 ms, menos as zonas de transição). Frame ausente conta 0. Trechos pulados por seek ficam fora (`Scorer::skip`).
- **A7: golden.** A regressão ganha `assert_eq!` do `ScoreResult` completo (valores exatos) para cada fixture. Qualquer mudança de resultado quebra o teste e exige bump de versão e atualização desta spec.
- **A9: fixtures realistas.** Notas repetidas ligadas, legato por semitom, notas curtas (30/50/100 ms), adiantado e atrasado ±300 ms, sirene, nota fixa e voz realista (formantes, jitter, shimmer, sopro), incorporadas de `qa_probe.rs` (Agente 4).
- **A2: voz-guia no microfone.** Não dá para resolver só no scorer. Decisão: a **checagem de vazamento** (SPEC-004 R9) passa a ser **obrigatória para modo competitivo**, por dispositivo e volume. O resultado fica salvo nas settings (`leakCheck: { deviceId, videoVolume, passed, atMs }`). Sem checagem aprovada para o dispositivo e o volume atuais, a performance é treino, e a UI explica e sugere fones. Tarefa T-004-06. Até ela existir, é um risco aceito e registrado no roadmap.

Critérios v1 que continuam valendo: toda a tabela acima, com as metas acrescentadas pelos itens acima.

**Tolerâncias v2** (T_full, T_zero, em cents): Easy (40, 200), Normal (25, 165), Hard (15, 155). O Normal ficou mais exigente que no v1: +50 cents dá 74,5. ⏳ Risco: calibrado só com voz sintética (incluindo drift de 15 cents, jitter e sopro: 99,6–100). Precisa ser confirmado com gravações reais de cantores amadores antes da release; se amadores afinados ficarem abaixo de ~85 no Normal, ajustar (com bump de versão).

## Algoritmo v3 (decisão do coordenador, 2026-11-19, a partir da re-revisão de QA)

Revisão: [scoring-review-v2.md](../docs/testing/evidence/scoring-review-v2.md). `ALGORITHM_VERSION = 3`. Resultados v1 e v2 já gravados ficam preservados e separados no ranking (Lei 11).

- **N1 (ALTA): vibrato não é desafinação.** O erro em cents passa a ser medido sobre o **contorno sem vibrato** (suavização de ~1 período, 150–200 ms, centrada; formulação do Agente 2), e não frame a frame. Metas, com voz realista centrada, Normal: vibrato ±50 c ≥ 95; ±75 c ≥ 92; ±100 c ≥ 88. Desafinação constante continua penalizada como no v2 (as metas de A4 seguem valendo).
- **N2 (ALTA): frame que não chegou ≠ frame calado.** Frame ausente (o áudio não chegou: déficit do driver, buffer cheio) fica **fora** do denominador, como `skip`. Frame que chegou e não é vozeado continua contando 0. Se mais de **5%** dos frames esperados da música faltarem, a performance vira **treino**, com reason em pt-BR ("o áudio do microfone teve falhas…"). Isso mantém A8 resolvido (perda grande ⇒ não competitivo) sem punir máquinas com pequeno déficit. O `performance.rs` mede a falta pelo contador do callback em relação ao tempo de vídeo tocado, e o `CaptureStats` registra a porcentagem.
- **N3 (MÉDIA): o gate não cobra o mesmo erro três vezes.** A acurácia usada no fator contínuo de ritmo/sustentação começa no onset detectado (ou em start + 80 ms se não houver onset), de modo que segurar a nota anterior pese uma vez só.
- **N4 (MÉDIA): notas curtas × latência.** Notas < 120 ms ganham tolerância de tempo proporcional (formulação do Agente 2), com meta: ligadas de 50 ms com 25 ms de atraso ≥ 70; de 100 ms ≥ 85. Além disso, a calibração assistida (T-004-07) passa a exibir a precisão (jitter) e recomenda repetir se ela for > 10 ms.
- **A3 (resto):** se a voz já está na nota-alvo no início da janela, o onset recua até o início real do trecho contínuo, para que entrar 400 ms antes não equivalha a entrar 190 ms antes. Regressão com pausas curtas (≤ 105 ms) e longas.
- **A4(b):** meta mantida em ≤ 10 por 10 cents (o QA mediu 10,7 entre 50 e 60 cents com frames ideais).
- **Gatilho de calibração com amadores:** a curva do QA (amador típico 85–90 no Normal) fica registrada. As tolerâncias de pitch **não** mudam no v3 sem evidência de voz real; o N1 já deve elevar o amador com vibrato.

## Algoritmo v4 (decisão do coordenador, 2026-11-19, a partir da 3ª rodada de QA)

Revisão: [scoring-review-v3.md](../docs/testing/evidence/scoring-review-v3.md). O v3 cumpriu todas as suas metas e **não** afrouxou a desafinação constante (o controle sem vibrato dá v2 ≈ v3). `ALGORITHM_VERSION = 4`.

- **E1 (ALTA): oscilar em volta do alvo não é afinar.** O contorno de 190 ms premia quem oscila em torno da nota sem nunca acertá-la: trinado ±100 c a 6 Hz → 95,8; senoide ±150 c → 93,5. Correção: medir também o **espalhamento** em torno do contorno (ex.: média de |frame − contorno|) e reduzir o crédito de afinação quando ele passar do que um vibrato natural produz (formulação do Agente 2; o QA sugere começar a reduzir em ~70 c e zerar em ~140 c). **Metas (voz realista, Normal):** N1 preservado (vibrato centrado ±50/±75/±100 ≥ 95/92/88); trinado quadrado ±100 c a 6 Hz ≤ 50; senoide ±150 c a 5,5 Hz ≤ 50; oscilação aleatória até ±150 c ≤ 50; alternância +1 st/+11 st ≤ 50. Todas as metas v3 seguem valendo.
- **E2 (BAIXA): frames descartados por ring cheio** (depois do callback) também entram no `missing_ratio`. Teste cruzando as duas camadas (scorer + performance.rs): "só os 100 ms iniciais de cada nota chegam" ⇒ treino.
- **E3 (BAIXA, teórico): perda seletiva < 5%** não é controlável pelo usuário no fluxo real (a perda vem do driver). Risco aceito e registrado; não muda o v4.

## Algoritmo v5 (decisão do coordenador, 2026-11-19, a partir da 4ª rodada de QA)

Revisão: [scoring-review-v4.md](../docs/testing/evidence/scoring-review-v4.md). O v4 resolveu o E1 sem regressões nem exploits novos, mas o termo de movimento também cobra **gestos expressivos monotônicos** (fall, bend, scoop lento, vibrato irregular), que custam 5 a 10 pontos. Estilo não é erro. `ALGORITHM_VERSION = 5`.

- **Movimento só conta quando é oscilação:** o centro **inverte de direção** repetidamente numa janela de ~0,5–1 s (ou outra formulação equivalente que separe vai-e-volta de gesto único; escolha do Agente 2).
- **Metas (voz realista, Normal, ornamento em todas as notas), no máximo 2 pontos abaixo do v3:** fall −300 c nos últimos 250 ms (v3 88,1); bend +100 c e volta em 300 ms (v3 96,8); scoop lento −100 c em 400 ms (v3 95,3); vibrato irregular 4–7 Hz / 30–90 c (v3 98,4).
- **Mantidas:** E1 ≤ 50 (atenção à margem da aleatória ±150), N1, todas as metas v3/v4 e a curva do amador.
- **Fora do escopo:** sopro com SNR 5 dB (caso extremo; SNR ≥ 10 dB está ileso) e ligadas de 100 ms com 50 ms de atraso, além da tolerância do N4 (a calibração de latência resolve).
- **Encerramento do ciclo sintético:** depois do v5, mudanças de pontuação só com **evidência de voz real** (roteiro do proprietário e gravações autorizadas), para não sobreajustar a vozes sintéticas.

## PLAN
`scoring-engine` puro. `Scorer` incremental; `finish()` consolida. 5 min ≈ 30k frames em memória (ok). `performance.rs` (src-tauri) faz âncora/sync, aplica latência e decide `competitive`.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-006-01 | `Scorer` v1 conforme fórmula + testes unitários | Agente 2 | done |
| T-006-02 | Regressão com gerador de fixtures sintéticas (`tests/regression.rs`) cobrindo a tabela | Agente 2 | done |
| T-006-03 | `performance.rs`: âncora, resync, pausa, seek→treino, latência | Agente 2 | done |
| T-006-04 | Relatório de precisão em `docs/testing/scoring-accuracy.md` | Agente 2 | done |
| T-006-05 | Persistência da performance + recordes | Agente 1 | done |
| T-006-06 | Algoritmo v2 (A1, A3–A9) + golden + fixtures realistas, `ALGORITHM_VERSION = 2` | Agente 2 | done |
| T-006-07 | Medição de desempenho do caminho de áudio (CPU/memória 5 min, latência frame → LiveTick, estabilidade em 10 execuções, custo do finish) → `docs/testing/performance.md` | Agente 2 | done |
| T-006-08 | Re-revisão independente de QA do algoritmo v2 → `docs/testing/evidence/scoring-review-v2.md` | Agente 4 | done |
| T-006-09 | Algoritmo v3 (N1–N4, resto do A3, A4b) + golden + regressão com as sondas v2, `ALGORITHM_VERSION = 3` | Agente 2 | done |
| T-006-10 | 3ª rodada de QA independente (v2 × v3) → `docs/testing/evidence/scoring-review-v3.md` | Agente 4 | done |
| T-006-11 | Algoritmo v4 (E1 espalhamento, E2 ring cheio no missing_ratio) + regressão/golden com as sondas v3, `ALGORITHM_VERSION = 4` | Agente 2 | done |
| T-006-12 | 4ª rodada de QA (curta, v3 × v4) → `docs/testing/evidence/scoring-review-v4.md` | Agente 4 | done |
| T-006-13 | Algoritmo v5 (movimento só como oscilação) + regressão com fall/bend/scoop lento/vibrato irregular, `ALGORITHM_VERSION = 5` | Agente 2 | done |
| T-006-14 | 5ª rodada de QA (final do ciclo sintético, v4 × v5) → `docs/testing/evidence/scoring-review-v5.md` | Agente 4 | done |
| T-004-06 | Checagem de vazamento obrigatória para modo competitivo (A2): medição no audio-engine/performance, settings `leakCheck`, tela no assistente de microfone | Agente 2 + Agente 1 + Agente 3 | done |

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
- T-006-06 `review` (Agente 2, 2026-11-19): algoritmo v2, `ALGORITHM_VERSION = 2`. Medições em [scoring-accuracy.md](../docs/testing/scoring-accuracy.md#algoritmo-v2-algorithm_version--2-atual). Formulação (constantes em `scorer.rs`):
  - **Tolerâncias (T_full, T_zero):** Easy (40, 200), Normal (25, 165), Hard (15, 155). As larguras ≥ 135 cents mantêm o salto ≤ 10 pontos em 10 cents. Normal: +90 cents = 37,1 e +100 cents com P = 0,46.
  - **Fator de acerto (A4 + A5):** `gate_i = clamp(2·acerto_i − 1, 0, 1) · min(1, 2·pitch_i)`, em que `acerto_i` = média de p nos frames vozeados da nota. Então `rhythm_i = timing·gate_i` e `sustain_i = sustentado_i·gate_i` (sustentado = frames vozeados com p > 0 sobre os esperados). Isso é contínuo, sem degrau em p = 0,5: o onset ainda usa p ≥ 0,5, mas o ritmo já vale 0 nesse ponto.
    - **Desvio da regra literal "rhythm_i = 0 se pitch_i < 0,5":** a regra literal + continuidade zeraria o ritmo de uma nota cantada afinada só na metade. O fator acima zera sirenes (9,5) e notas desafinadas, mas mantém o ritmo de quem cantou parte da nota afinado.
  - **Onset (A1 + A3):** janela `[max(start − 300, min(fim_anterior + 25 ms, start)), start + d/2]`.
    - Os 25 ms (meia janela de análise) são o rastro da nota que acabou, nunca um ataque. Sem eles, F4 depois de E4 dava R = 0,46 com canto perfeito.
    - O onset também exige que o canto esteja mais perto deste alvo que do alvo anterior: quem ainda está na nota anterior não "chegou". Sem isso, atrasado e adiantado 100 ms saíam assimétricos.
  - **Zonas de transição (A6)** como especificado. A mediana de 5 frames usa só frames da própria nota (sem vazar a vizinha ligada).
  - **Denominador (A8):** `max(floor(região/10 ms), frames recebidos)`. Limite: uma nota que começa exatamente em 0 ms (ou no alvo de um seek) perde ≤ 21 ms, porque o primeiro frame é centrado meia janela depois do início da captura.
  - **Golden (A7):** 7 fixtures com `ScoreResult` exato. **Fixtures (A9):** as do Agente 4 incorporadas a `regression.rs` (voz humana com formantes, jitter, shimmer, drift e sopro; ligadas; curtas; ±300 ms; sirene; nota fixa; voz-guia).
- Metas v2 não atingidas (evidência em scoring-accuracy.md, aguardando decisão do coordenador):
  - (b) "≤ 5 pontos por 10 cents" é matematicamente incompatível com (c) e com perfeito ≈ 100. Implementado ≤ 10 (medido: 9,7).
  - "Nota fixa ≤ 15": medido 24,4. Ela canta de fato 2 das 10 notas. A regressão afirma ≤ 25.
- Tarefas review → done (Agente 0, 2026-11-19): integrado em main @ 566a64e; fmt, clippy --workspace -D warnings e cargo test --workspace verdes (80 testes).
- T-006-05 `review` (Agente 1, 2026-11-19): `src-tauri/src/live.rs` registra `performance.rs` e expõe `start_performance` (para o medidor, substitui uma execução esquecida, valida música, participante e rodada (sessão ativa, rodada não concluída, música/participante batem), monta `PerformanceConfig` com a referência ativa e as settings, explica em `reason` por que não é competitivo), `sync_performance`, `finish_performance` e `abort_performance`. Persistência: com referência sempre grava (competitiva ou treino); sem referência não grava nada e devolve as métricas exploratórias. `competitive` final = `summary.competitive && played_ms ≥ 50% de duration_ms` (duração desconhecida = treino). Rodada vira `completed` só com performance competitiva e sessão ativa. `personalBest`/`songRecord` na CompatKey da execução (referência e revisão do início). `audio://error`: watchdog a cada 250 ms no medidor e na performance; ao falhar, o stream é parado (a performance é descartada) e o evento é emitido. Testes ✅ `live::tests::{prepare_reports_competitiveness, competitive_needs_half_the_song, unknown_duration_is_training_and_no_reference_stores_nothing, round_completes_and_records_per_participant}`; os 9 testes de `performance.rs` passam dentro do app; `performance::tests::live_run_on_the_default_microphone` (`--ignored`) passou no microfone padrão desta máquina.
- T-006-03 e T-006-05 done (Agente 0, 2026-11-19): integrado em main @ 1d88d64; fmt, clippy --workspace -D warnings e cargo test --workspace verdes (112 testes, app 40 + 1 ignorado de hardware). ⏳ start→sync→finish via IPC real ainda não exercitado no app.

- T-006-06 done (Agente 0, 2026-11-19): integrado em main @ 94fecbe; 70 testes de áudio/pontuação (golden incluído) + 40 do app verdes, clippy limpo. Emendas aceitas registradas na seção v2. ⏳ Re-revisão do Agente 4 e validação com voz real.
- T-004-06 parte Agente 1 `review` (2026-11-19):
  - `settings.leakCheck {deviceId, videoVolume, passed, atMs}`. É só leitura em `update_settings` (`invalid_setting`) e é gravado só pela execução de teste.
  - `start_performance` com `leakCheck: true`: recusa rodada/participante; exige referência ativa experimental ou validated; `PerformanceConfig.leak_check`.
  - No finish: nada vai para `performances`; o veredito vai para as settings (um veredito reprovado substitui uma aprovação anterior) e é devolvido em `outcome.leak`.
  - Gate competitivo: `leakCheck.passed && deviceId == inputDeviceId && videoVolume ≤ leakCheck.videoVolume + 0,05`. Senão é treino, com o reason "Faça o teste de vazamento do microfone (ou use fones) para pontuar no ranking.".
  - Testes ✅ `live::tests::{leak_check_gates_competitive, leak_check_run_saves_verdict_only, leak_check_is_read_only_for_update_settings}`.
- T-004-06, parte da UI (Agente 3, 2026-11-19), em review:
  - Assistente de Microfone (`src/screens/LeakTest.tsx`): escolhe a música (referência validada, senão experimental; disponível e não `playable='no'`). Para o monitor, espera os metadados, posiciona o vídeo na 1ª nota (`startMs + offsetMs`) no volume atual e chama `startPerformance({leakCheck:true})`. Toca 10 s com sync de `mediaTime` (helper `src/player/sync.ts`, que o Player também usa), pausa, chama `syncPerformance(t, false)` e `finishPerformance()`, e mostra `outcome.leak.passed` (aprovado, ou "use fones de ouvido ou baixe o volume"). Ao terminar, recarrega as settings (o `leakCheck` salvo pelo Rust) e religa o monitor. Em erro, chama `abortPerformance` se a performance ficou aberta. Ganhou também o slider "Volume da música" (`videoVolume`), para testar no volume de uso.
  - Estado do teste: `leakCheckState` (`src/player/leak.ts`) espelha a regra do Rust: aprovado só com o mesmo `inputDeviceId` e `videoVolume` ≤ ao do teste; senão "Refazer" ou "Reprovado".
  - Player: música validada sem teste aprovado mostra o aviso e o botão "Fazer teste" (vai para Microfone); o botão principal vira "Começar como treino". O mesmo atalho aparece na pausa, junto do `reason` do Rust.
  - Testes: `LeakTest.test.tsx` (4), `leak.test.ts` (+4 de `leakCheckState`), `Player.test.tsx` (+1).
  - Regra igual à do Agente 1, inclusive a tolerância de +0,05 no volume; o texto do `reason` não é interpretado. ⏳ Testes de UI com IPC simulado; com os commands reais (main 2cef2b0) só verificado o tipo.

- T-004-06 (Agente 0, 2026-11-19): partes dos Agentes 2 e 1 integradas em main @ 2cef2b0 (140 testes Rust verdes). Decisão: qualquer veredito reprovado (inclusive inconclusivo) substitui uma aprovação anterior, que é o lado conservador. A UI (Agente 3) entra no próximo merge.
- T-004-06 done (Agente 0, 2026-11-19): integrado em main @ 52a8b95; typecheck, vitest 158/158, Playwright 43/43 + test-song 1/1, cargo test -p game 16 verdes. Ponta a ponta: Rust (gate + verdict) + UI (assistente + aviso no Player). ⏳ medição real com caixas.
- T-006-07 `review` (Agente 2, 2026-11-19): medição de desempenho em [performance.md](../docs/testing/performance.md), com os testes `perf_*` (`#[ignore]`) no `performance.rs`.
  - Análise: 2,0–2,3% de um núcleo (meta < 15%). Processo ao vivo: 3,8% (release).
  - Callback → `on_tick`: p95 5,6 ms, p99 8,0 ms. Som → `on_tick` do feedback: p99 ≈ 49 ms.
  - 10 performances seguidas: sem crescimento de threads nem de memória.
  - `finish`: cálculo 0,03–0,15 ms, mas liberar o dispositivo leva 43–503 ms (cpal/WASAPI). Há uma proposta, aguardando o coordenador.
- T-006-07 done (Agente 0, 2026-11-19): integrado em main @ 1e7ddb2. Todas as metas de áudio cumpridas (CPU 3,8% release; callback→tick p95 5,6 ms; sem vazamento em 10 execuções). Decisão: a liberação assíncrona do dispositivo no finish foi RECUSADA (corrida com start, sem meta violada); o finish já roda em `blocking`. ⏳ latência Channel → pintura (UI).
- T-006-08 `review` (Agente 4, 2026-11-19): re-revisão de QA do v2 em [scoring-review-v2.md](../docs/testing/evidence/scoring-review-v2.md), com v1 × v2 no mesmo áudio.
  - Resolvidos: A1, A4 (monotônico; +90 c = 37,1), A5 (sirene ≤ 11,8; nota fixa 24,4), A6, A7, A8 e A9 em boa parte. A2 resolvido por processo (T-004-06).
  - Parcial: A3 (assimétrico com pausa < 300 ms; o teste A3 usa só pausas de 400 ms). A4(b): queda máxima 10,7 (Normal) e 10,4 (Hard), contra a meta de ≤ 10.
  - Novos achados: N1 (Alta) vibrato ±75/±100 c → 77,5/63,9 com o cantor centrado; N2 (Alta, ❓) com o denominador fixo, 5–15 % de frames perdidos custam 4–12 pontos, e o WASAPI desta máquina entregou 40–46 mil de 48 mil amostras/s; N3 (Média) o gate pune duas vezes o atraso legato; N4 (Média) notas de 50 ms com 25 ms de atraso → 1,1.
  - Amador típico (drift ±20, vibrato ±50, desvio ±20 c) no Normal: 85,6–90,4.
- T-006-08 done (Agente 0, 2026-11-19): re-revisão integrada; achados N1–N4 → algoritmo v3 (T-006-09).
- T-006-09, parte da UI (Agente 3, 2026-11-19), em review: Resultado com `competitive=false` mostra o selo "Treino — fora do ranking" e o `outcome.reason` do backend em destaque, também quando não há referência. O atalho vem de `trainingAction` (pura), pelo estado e não pelo texto: sem referência → "Importar referência"; não validada → "Abrir referência"; teste de vazamento não aprovado (`leakCheckState`) → "Fazer teste de vazamento". Seek, falha de áudio ou pouca música tocada não têm atalho ("Cantar de novo" resolve). Testes: `Result.test.tsx` (7).

- T-006-09 `review` (Agente 2, 2026-11-19): algoritmo v3, `ALGORITHM_VERSION = 3`. Medições v2 × v3 no mesmo áudio (voz humana) em [scoring-accuracy.md](../docs/testing/scoring-accuracy.md#algoritmo-v3-algorithm_version--3-atual): todas as metas do v3 atingidas, e a tabela v1 continua passando. Formulação (constantes em `scorer.rs`):
  - **N1, contorno sem vibrato:** a nota de cada frame é a média das medianas de 5 frames numa janela de 19 frames (190 ms, cerca de um período de vibrato a 5,3 Hz). A janela fica dentro da nota e não cruza o onset (N3). Só entram valores a até 250 c do frame, então ornamentos curtos e distantes não contaminam os vizinhos. O feedback ao vivo continua frame a frame; o rótulo usa a mediana do erro nos últimos 200 ms.
  - **Mediana com oitava:** antes da mediana, os valores são dobrados para a oitava do frame. O YIN salta oitavas em trocas rápidas com voz de formantes; sem isso, ligadas de 50 ms com 25 ms de atraso davam 46 com voz humana.
  - **N2:** o denominador é o número de frames que chegaram. Os frames esperados depois do último recebido contam 0 (a execução parou, não houve falha). Nota sem nenhum frame fica fora, como `skip`. Mais de 5% de áudio faltando → treino (`performance.rs`, integrado em main @ 77c98ff).
  - **N3:** a acurácia do gate vai do onset desta nota (ou start + 80 ms) até o onset da próxima. Atraso e adiantamento pesam uma vez só.
  - **N4:** nota com d < 120 ms usa a melhor região deslocada até min(120 − d, 30) ms, em passos de 10 ms. A janela de onset também se estende esse tanto.
  - **A3:** se a voz já está no alvo quando a janela abre, o onset recua até o início real do trecho, atravessando a pausa e o fim da nota anterior se o canto estiver estritamente mais perto deste alvo, até start − 300 ms. Se o cantor nunca fica mais perto deste alvo que do anterior (desafinado em direção à nota anterior), vale o primeiro frame no alvo. Isso eliminou a queda de 10,9 entre 50 e 60 c (A4(b): 9,7).
  - Cada nota é pontuada uma vez, quando os frames chegam a fim + 60 ms. O `running_score` cobre essas notas. Frames fora de ordem (re-âncora para trás) são descartados.
  - Golden com 12 fixtures. A música de teste T-003-05 foi congelada em `tests/fixtures/references/test-song.json`.
  - ⏳ As tolerâncias de pitch não mudaram. A curva do amador subiu (Normal: desvio 0 → 100,0; +20 c → 96,7; −40 c → 88,2). Confirmar com voz real antes da release.
- T-006-09 done (Agente 0, 2026-11-19): algoritmo v3 integrado (`ALGORITHM_VERSION = 3`) + UI do motivo do treino; `pnpm check` exit 0 (regressão 29 com golden de 12 fixtures), Playwright 99/99. Todas as metas v3 cumpridas com voz realista (vibrato ±75 c: 77,4 → 99,9; 10% de perda: 90,6 → 100; ligadas de 50 ms com 25 ms de atraso: 2,0 → 100). ⏳ 3ª rodada de QA (Agente 4); voz real.
- T-006-10 `review` (Agente 4, 2026-11-19): 3ª rodada de QA em [scoring-review-v3.md](../docs/testing/evidence/scoring-review-v3.md), com v2 × v3 no mesmo áudio.
  - Metas do v3 atingidas: N1 (vibrato ±50/±75/±100 → 100/99,9/99,7), N2 (5–15 % de frames perdidos → 100), N3, N4 (50 ms +25 ms → 95,8; 100 ms +25 ms → 99,5), A3 simétrico, A4(b) (máx. 9,7).
  - Novo achado E1 (Alta): o contorno de 190 ms premia oscilar em torno do alvo. Trinado ±100 c a 6 Hz, sem nunca cantar o alvo: 95,8 pelo pipeline; senoide ±150 c: 93,5. Ambos acima de quem canta firme a +30 c (90,8).
  - E2 (Baixa): o A8 passou a depender do limite de 5 % do `performance.rs`; no scorer, "só os 100 ms iniciais" dá 91,1. E3 (Baixa): perda seletiva < 5 % rende +3,2.
  - Leniência: sem vibrato, v2 = v3. O Normal distingue 0/30/50 c → 100/90,8/72,9 com vibrato.
- T-006-10 done (Agente 0, 2026-11-19): 3ª rodada integrada; v3 confirmado em todas as metas e sem afrouxar a desafinação constante. E1 (oscilação premiada) → v4, T-006-11.
- T-006-11 `review` (Agente 2, 2026-11-19): algoritmo v4, `ALGORITHM_VERSION = 4`. Tabela v3 × v4 no mesmo áudio (voz humana) em [scoring-accuracy.md](../docs/testing/scoring-accuracy.md#algoritmo-v4-algorithm_version--4-atual). Todas as metas do v4 e do v3 atingidas; a tabela v1 continua passando.
  - **E1:** erro por frame = `|contorno − alvo| + 2,5·max(0, espalhamento − 60 c) + 2·max(0, movimento − 20 c)`. Espalhamento = média de |mediana − contorno| na janela de 190 ms. Movimento = |contorno(+100 ms) − contorno(−100 ms)|. O termo de movimento é necessário porque a oscilação aleatória ±150 c tem RMS igual ao de um vibrato ±100 c e espalha menos; o que a distingue é o centro que anda.
  - Resultado: trinado ±100 c 44,4; senoide ±150 c 43,0; aleatória ±150 c 44,2; alternância +1/+11 st 40,4. N1 preservado (100,0 / 99,9 / 99,7).
  - Movimento legítimo (pedido do coordenador, na regressão):
    - scoop de 150 c custa 2,3 pontos (v3: 1,7);
    - vibrato ±75 c a 4 Hz dá 97,7;
    - drift de ±30 c dentro de notas de 2 s fica ≥ 96,9 (≤ 2,3 abaixo do v3);
    - glissando de 80 ms 97,7; N4 sem mudança.
  - Desafinação constante idêntica ao v3 (frames ideais) e, com voz humana, igual a menos de 0,1.
  - O contorno também não cruza o onset da próxima nota. O recuo do onset (A3) vai até 1 s para cair na troca real.
  - **E2:** `Capture::dropped_samples()` (contracts §2.1) conta as amostras perdidas com o ring cheio. O `performance.rs` as desconta, e o `missing_ratio` passa a incluí-las. Teste que cruza as duas camadas: só os 100 ms iniciais de cada nota chegam → o scorer sozinho dá ≥ 80 e a sessão marca treino.
  - **E3:** risco aceito, sem mudança.
- T-006-11 done (Agente 0, 2026-11-19): algoritmo v4 integrado (`ALGORITHM_VERSION = 4`); `pnpm check` exit 0 (regressão 31 com golden de 14 fixtures). E1: trinado/senoide/aleatória/alternância 93–97 → 40–44; N1 preservado; casos legítimos (scoop, glissando, vibrato lento, drift) custam ≤ 2,3. ⏳ 4ª rodada de QA; voz real.
- T-006-12 `review` (Agente 4, 2026-11-19): 4ª rodada em [scoring-review-v4.md](../docs/testing/evidence/scoring-review-v4.md).
  - E1 ✅: trinado 44,2 (pipeline), senoide 32,4, aleatória 44,5, alternância 28,1.
  - E2 ✅ por leitura do teste de sessão.
  - Metas v3 todas mantidas; curva do amador igual (≤ 0,2).
  - Achado Média: o termo de movimento penaliza gestos expressivos monotônicos (fall −9,6; bend −10,2; scoop lento −5,3; vibrato irregular −7,0, com o ornamento em todas as notas) e voz com SNR 5 dB no agudo (−13,9). Nenhum exploit novo.
- T-006-12 done (Agente 0, 2026-11-19): 4ª rodada integrada. v4 confirmado (E1/E2 ok, sem regressão). O teste E2 do app passou no `pnpm check` do coordenador (57 testes do app). Falsos positivos em gestos expressivos → v5 (T-006-13).
- T-006-13 `review` (Agente 2, 2026-11-19): algoritmo v5, `ALGORITHM_VERSION = 5`. Tabela v4 × v5 no mesmo áudio (voz humana) em [scoring-accuracy.md](../docs/testing/scoring-accuracy.md#algoritmo-v5-algorithm_version--5-atual-encerra-o-ciclo-de-ajuste-com-voz-sintética).
  - **Movimento só como oscilação:** o termo do v4 virou `3·max(0, balanço − 50 c)`. Balanço = máximo − mínimo do centro em ±600 ms; centro = contorno promediado de novo em 250 ms (a cascata 190 + 250 ms deixa < 3% de um vibrato de 4–7 Hz). Só conta se o contorno inverter de direção ≥ 2 vezes (histerese de 8 c) na mesma janela. O peso do espalhamento subiu de 2,5 para 3,5.
  - **Gestos** (no máximo 2 abaixo do v3): scoop lento 95,3, fall 88,1, bend 96,8 (iguais ao v3); vibrato irregular 98,1 (v3 98,4).
  - **E1:** ≤ 41,1 (aleatória ±150: 41,1, margem 8,9).
  - **Mantidos:** N1, metas v3/v4, amador e curva constante sem mudança.
  - Na regressão: `v5_expressive_gestures_are_style_not_error`. Golden com 14 fixtures.
  - A janela de oscilação (1,2 s) ficou um pouco acima dos "~0,5–1 s" da spec. Com 1 s, a aleatória ±150 em notas de 1 s quase nunca mostra 2 inversões, e o E1 não fecha.
  - **Fim do ciclo sintético:** mudanças de pontuação a partir daqui só com evidência de voz real.
- T-006-13 done (Agente 0, 2026-11-19): algoritmo v5 integrado (`ALGORITHM_VERSION = 5`); `pnpm check` exit 0 (regressão 32, golden de 14); os gestos expressivos voltaram aos valores do v3; E1 ≤ 44,2. Desvio aceito: janela de oscilação de ±600 ms (justificada). Corrigida também uma corrida pré-existente no SongDetails.test. ⏳ Rodada 5 de QA (final do ciclo sintético).
- T-006-14 `review` (Agente 4, 2026-11-19): 5ª rodada em [scoring-review-v5.md](../docs/testing/evidence/scoring-review-v5.md). Nenhum achado Alto.
  - Metas ✅: gestos 88,1/96,8/95,3/98,1 (≤ 0,3 abaixo do v3); E1 trinado 26,5, senoide 28,6, aleatória 39,4, alternância 9,6; v3/v4 e curva do amador idênticos.
  - Médios registrados para a calibração com voz real: a janela de ±600 ms do balanço zera frames afinados em volta de ornamentos de 200 c (volteio 39,1; trinado de fim de frase 49,0). Oscilações mais lentas que as metas do E1 passam pouco do teto (triângulo ±150 c a 1 Hz 58,5; serra 53,4), abaixo de constante +60 c (66,2).
- T-006-14 done (Agente 0, 2026-11-19): rodada 5 (final do ciclo sintético), sem achados ALTOS. v5 aprovado para a 0.1.1.
- **Backlog para a calibração com voz real** (não mexer sem evidência real, conforme decisão do v5): (M1) ornamento de ~200 c dentro de nota longa zera também os frames afinados vizinhos (volteio 68,7 → 39,1; trinado de fim de frase 73,1 → 49,0); sugestão: aplicar o balanço só entre a 1ª e a última inversão. (M2) oscilações lentas passam pouco do teto do E1 (triângulo ±150 c a 1 Hz 58,5; serra 53,4), sem inverter o ranking. (B1) glissando único através do alvo paga pela distância (coerente com "gesto = estilo"). Evidência: docs/testing/evidence/scoring-review-v5.md.
