# Desempenho do caminho de áudio (T-006-07)

Dono: Agente 2. Metas: [strategy.md](strategy.md#performance-metas-a-medir-números-só-após-medição-no-ambiente-de-referência). Medido em 2026-11-19.

**Ambiente:** Intel Core i5-11400H (6 núcleos, 12 threads), 31,8 GB RAM, Windows 11 Home, rustc 1.99.1, microfone "Grupo de microfones (Realtek(R) Audio)" a 48 kHz (WASAPI compartilhado).
**Carga concorrente:** outros agentes compilavam durante as medições (carga total de 25% a 100%, registrada em cada execução). Os números são, portanto, pessimistas.
**Builds:**
- `release`: perfil release do workspace.
- `dev`: `audio-engine` e `scoring-engine` com `opt-level = 3`, como no `tauri dev`; o resto em debug.

**Como reproduzir:** testes `#[ignore]` em `apps/desktop/src-tauri/src/performance.rs` (os nomes começam com `perf_`).
- Comando: `cargo test -p master-of-notes perf_ -- --ignored --nocapture --test-threads=1`. Acrescente `--release` para o build release.
- `PERF_MINUTES` e `PERF_RUN_SECONDS` encurtam as execuções.
- Estatísticas do processo vêm de `Get-Process` (as mesmas do Gerenciador de Tarefas).
- Nesta medição os testes rodaram num crate-harness que inclui o `performance.rs` via `#[path]`, pela regra de disco (o Agente 2 não compila o app).

## Resumo

| Métrica | Meta | Medido | ✅ |
|---|---|---|---|
| CPU da análise (YIN) | < 15% de um núcleo | 2,0% (dev) / 2,3% (release), offline, 5 min | ✅ |
| CPU do processo em performance ao vivo de 5 min (captura + análise + scorer + driver + laço de medição) | < 15% de um núcleo | 3,8% (release) / 4,6% (dev) | ✅ |
| Latência callback → `on_tick` (release, 5 min) | parte da meta "frame → UI < 80 ms" | p50 2,9 · p95 5,6 · p99 8,0 · máx. 47 ms | ✅ |
| Latência som → `on_tick` do feedback (estrutural + medida) | idem | p50 44 · p95 47 · p99 49 ms | ✅ (sobram ~30 ms para IPC + render, ver abaixo) |
| Memória em 5 min ao vivo | sem crescimento anormal | +0,7 MB do meio ao fim (frames do scorer, esperado) | ✅ |
| 10 performances seguidas | sem crescimento de memória nem de threads | threads estáveis em 7; memória em platô de 2,1 MB | ✅ |
| `finish()`, cálculo | — | 0,03 ms (offline) / 0,05–0,15 ms (ao vivo) | ✅ |
| `finish()`, total (inclui liberar o dispositivo) | — | **43–503 ms**, quase tudo dentro do cpal/WASAPI | ⚠️ proposta abaixo |

## (a) CPU e memória

**Offline, 5 min** (`perf_offline_5_minutes`): 300,5 s de melodia cantada (tom de 4 harmônicos sobre uma referência de 500 notas), 30 046 frames, pela análise real e pela sessão.

| Build | Análise | Sessão + scorer | `finish` (summary) | Score |
|---|---|---|---|---|
| dev | 5,88 s = 1,96% de um núcleo | 0,076 s = 0,025% | 0,030 ms | 100 (500/500) |
| release | 6,92 s = 2,30% | 0,035 s = 0,012% | 0,029 ms | 100 (500/500) |

A análise é igual nos dois builds, porque já roda com `opt-level = 3` no dev. A diferença entre eles é ruído da carga concorrente.

**Ao vivo, 5 min** (`perf_live_5_minutes_cpu_memory_and_tick_latency`, microfone real, syncs a cada 250 ms como a UI):

| Build | Duração | CPU do processo | Memória privada (início → meio → fim) | Threads | Ticks |
|---|---|---|---|---|---|
| release | 300 s | 11,5 s = 3,83% de um núcleo | 2,1 → 5,5 → 6,2 MB | 4 → 6 | 9 903 (33,0/s) |
| dev | 121 s | 5,5 s = 4,58% | 2,2 → 5,2 → 5,5 MB | 5 → 8 | 4 009 (33,2/s) |

- A CPU inclui o laço de medição, que amostra o contador a cada 0,5 ms. O custo real do produto é menor.
- Do meio ao fim, a memória cresce 0,7 MB, o que bate com os ~30 mil `VoiceFrame` (24 B) que o `Scorer` guarda numa música de 5 min. O resto da memória é fixo.

## (b) Latência callback → `on_tick`

**Método:**
- Um laço amostra `Capture::captured_samples()` (o contador do callback) a cada ~0,5 ms. Cada mudança é guardada com a última amostragem que ainda via o valor antigo e a primeira que vê o novo, e o callback entregou aquelas amostras nesse intervalo.
- Cada tick registra o instante e a última amostra do frame que o gerou.
- Latência = instante do tick − meio do intervalo em que o contador passou dessa amostra. Intervalos maiores que 5 ms (laço travado) são descartados e contados à parte.

| Build, duração, carga | p50 | p95 | p99 | máx. | n (descartados) | Período do driver p50 / p99 |
|---|---|---|---|---|---|---|
| release, 5 min, 70% | 2,9 ms | 5,6 | 8,0 | 47,2 | 9 332 (495) | 10,2 / 25,0 ms |
| dev, 2 min, 57–100% | 3,2 ms | 12,4 | 34,3 | 123,8 | 3 746 (206) | 10,2 / 24,3 ms |

**Experimentos de profiling** (release, 2 min cada, no método anterior, que tinha um viés de +~5 ms; servem só para comparar entre si):

| Experimento | Carga | p50 | p95 | p99 |
|---|---|---|---|---|
| A: base | 88% | 7,4 | 11,8 | 21,8 |
| B: processo em prioridade alta | 40% | 7,3 | 11,4 | 12,1 |
| C: espera ociosa da thread de análise de 1 ms (em vez de 5) | 25% | 5,5 | 11,3 | 24,3 |

Conclusões:
- A **mediana** é dominada pela espera ociosa de 5 ms da thread de análise. Com 1 ms, cai ~2 ms.
- A **cauda** (p99) é escalonamento do SO sob carga: some com prioridade alta.
- Os **máximos** de 130–190 ms do método antigo eram artefato: o laço de medição travava ~300 ms quando chamava o PowerShell. Isso foi corrigido.

**Do som ao `on_tick`:** o tick emitido ao analisar o frame k leva o feedback do frame k−2, porque a mediana do scorer precisa de 2 frames à frente. Somam-se:
- metade da janela de análise: 21,3 ms;
- o lookahead: 20 ms;
- callback → `on_tick`: p50 2,9 / p95 5,6 / p99 8,0 ms.

Total: **p50 44 · p95 47 · p99 49 ms**. O medidor (`LiveTick.mic`) chega 20 ms antes disso.

**Não medido aqui (⏳):**
- O buffer de captura do WASAPI antes do callback (tipicamente 10–20 ms em modo compartilhado). Ele entra na calibração de latência (T-004-07), não no relógio da UI.
- O caminho `Channel` → React → pintura (Agentes 1 e 3). Para caber na meta de 80 ms, sobram ≈ 30 ms no p95, cerca de dois quadros a 60 Hz.

## (c) Estabilidade: 10 performances seguidas

`perf_stability_10_runs`, release, 10 × 30 s com `LivePerformance` real (start → syncs → pausa → finish), carga de 100%.

| Execução | Threads (durante / depois) | Memória privada (durante / depois, MB) | Liberação do dispositivo | summary |
|---|---|---|---|---|
| 1 | 9 / 8 | 2,5 / 1,8 | 144,0 ms | 0,07 ms |
| 2 | 7 / 5 | 2,5 / 1,7 | 143,9 ms | 0,11 ms |
| 3 | 9 / 7 | 2,6 / 1,8 | 259,8 ms | 0,10 ms |
| 4 | 9 / 7 | 2,6 / 1,8 | 425,1 ms | 0,10 ms |
| 5 | 9 / 7 | 2,6 / 1,9 | 429,7 ms | 0,08 ms |
| 6 | 9 / 7 | 2,8 / 2,1 | 502,8 ms | 0,06 ms |
| 7 | 9 / 7 | 2,8 / 2,1 | 105,2 ms | 0,08 ms |
| 8 | 9 / 7 | 2,8 / 2,1 | 74,2 ms | 0,12 ms |
| 9 | 9 / 7 | 2,8 / 2,1 | 54,2 ms | 0,09 ms |
| 10 | 9 / 7 | 2,8 / 2,1 | 369,1 ms | 0,09 ms |

Linha de base antes da 1ª execução: 5 threads, 1,2 MB.

- **Threads:** a nossa thread de análise e a thread do stream do cpal são aguardadas (`join`) no drop. O que sobra é o pool de threads do Windows usado pelo WASAPI. Em quatro séries ele estabilizou em 7, 7, 10 e 7; um vazamento somaria uma thread por execução. O teste aceita até 5 a mais que após a 1ª execução.
- **Memória:** platô de 2,1 MB nas execuções 6–10. O teste falha se crescer ≥ 5 MB.

## (d) Custo do `finish()`

- O cálculo (`summary` + `Scorer::finish`) numa música de 5 min leva **0,03–0,15 ms**.
- **Liberar o dispositivo** leva **43–503 ms**. Medição temporária dentro do `audio-engine` (revertida) mostrou que 43–116 ms disso são o `drop` do stream do cpal, ou seja, a liberação do cliente WASAPI. A nossa parte (sinal de parada + espera ociosa de até 5 ms + `join`) fica em 3–26 ms. Sob carga de 100%, o total chegou a 0,5 s.

**Proposta (não implementada, aguardando o coordenador):** o `LivePerformance::finish` marca a sessão como encerrada (frames posteriores são ignorados), devolve o summary na hora e libera o dispositivo numa thread separada. O `finish` cai para < 1 ms. O custo: o próximo `start` pode concorrer com uma liberação ainda em andamento (o WASAPI permite; confirmar com o teste de estabilidade). Até lá, os commands devem chamar `finish`/`abort` fora da thread principal (command `async` ou `spawn_blocking`) para a UI não travar.

## Melhorias opcionais (nenhuma meta exige)

- Acordar a thread de análise com `Thread::unpark` no callback, em vez de esperar 5 ms ociosa: −2 a −3 ms na mediana (experimento C).
- Prioridade "Pro Audio" (MMCSS) para a thread de análise: corta a cauda sob carga (experimento B).
