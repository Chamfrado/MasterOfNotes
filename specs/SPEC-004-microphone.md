# SPEC-004 — Captura, análise e calibração do microfone

Contrato: [contracts.md §2.1](../docs/architecture/contracts.md#21-audio-engine-agente-2).

## SPEC
- R1. Listar dispositivos de entrada; selecionar e persistir (`inputDeviceId`; `null` = padrão do sistema).
- R2. Medidor ao vivo (nível, pitch detectado, clipping) na tela de configuração (~30 Hz).
- R3. Análise: mono (média dos canais); janela ~43 ms (2048 amostras @ 48 kHz, proporcional a outras taxas); hop 10 ms; YIN com limiar 0,15 e interpolação parabólica; faixa 70–1100 Hz.
- R4. `voiced` = `rms_dbfs ≥ noise_floor + gate_margin (10 dB)` **e** `confidence ≥ 0,6` **e** `f0` presente. Acima do gate, o volume não altera nada (volume ≠ qualidade).
- R5. `clipped` = alguma |amostra| ≥ 0,99 no frame.
- R6. Calibração de ruído: N s em silêncio → noise floor = percentil 90 do RMS; veredito `noisy` se > −40 dBFS, `clipping` se > 1% frames clipados, `too_quiet` se o teste de voz não passar do gate.
- R7. Latência: ajuste manual (−500…+500 ms) com teste de palmas sobre clique; assistente automático é tarefa posterior.
- R8. Análise fora do callback de áudio (ring buffer SPSC → thread de análise). O callback não aloca nem bloqueia.
- R9. Vazamento: com a música tocando e o usuário em silêncio, se > 20% dos frames forem `voiced`, avisar "o microfone está captando a música — use fones".

Critérios:
- Given senoide 440 Hz, Then f0 ∈ [438, 442] em ≥ 95% dos frames.
- Given silêncio digital ou ruído branco, Then `voiced=false` em ≥ 99% dos frames.
- Given a mesma senoide a −30 dBFS e −6 dBFS (acima do gate), Then mesmos `f0` e `voiced`.
- Given voz sintética (harmônicos) a 110 Hz, Then f0 ∈ ±1% (sem erro de oitava) em ≥ 95% dos frames.

## PLAN
`cpal` → ring buffer SPSC (`rtrb`) → thread de análise → callback do consumidor. `analyze_buffer` puro usa o mesmo código para testes offline.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-004-01 | YIN + VAD + `analyze_buffer` + testes sintéticos | Agente 2 | done |
| T-004-02 | `Capture` com cpal (WASAPI) + listagem de dispositivos | Agente 2 | done |
| T-004-03 | Calibração de ruído | Agente 2 | done |
| T-004-04 | Commands `list_input_devices`, `start/stop_mic_monitor`, `calibrate_noise` | Agente 1 | done |
| T-004-05 | Tela de microfone e calibração | Agente 3 | done |
| T-004-07 | Medição assistida de latência (sonda de cliques pelo pipeline do vídeo) | Agente 2 + Agente 1 + Agente 3 | doing (backend done; UI pendente) |

## TEST
`audio-engine/tests/synthetic.rs`: senoides, oitavas, glissando, silêncio, ruído, clipping, invariância a volume, harmônicos. Dispositivo real = teste manual registrado em `docs/testing/evidence/`.

## STATUS
- T-004-01 `review` (Agente 2, 2026-11-19): YIN + gate + clipping + `analyze_buffer` em `crates/audio-engine`. 15 testes sintéticos verdes. Todos os critérios acima medidos em [scoring-accuracy.md](../docs/testing/scoring-accuracy.md). CPU ≈ 2,7% de um núcleo.
- Detalhes de implementação: `clipped` olha só o hop do frame (cada amostra conta uma vez para o % de clipping). `f0_hz = None` quando confiança < 0,6, então `voiced = gate && f0.is_some()`. O tempo do frame é o centro da janela: `sample_index + window_len/2` (`audio_engine::window_len`).
- Limitação conhecida: tom acima de 1100 Hz é lido como o subharmônico dentro da faixa (2 kHz → 1 kHz). Não afeta a pontuação (dobra de oitava).
- T-004-02/03 `review` (Agente 2, 2026-11-19):
  - `Capture`: cpal 0.15 (WASAPI) → ring SPSC `rtrb` (2 s) → thread `mic-analysis`, que também é dona do stream (por isso `Capture: Send`). O callback só faz downmix e `push`: não aloca nem bloqueia; ring cheio descarta amostras;
  - `list_input_devices`: o id é o nome do dispositivo (cpal 0.15 não tem id estável);
  - `take_error` informa erro do driver;
  - `calibrate_noise`/`check_voice` são puras (p90 do RMS; Noisy > −40 dBFS; Clipping > 1%; TooQuiet abaixo do gate).
- Dispositivo real (2026-11-19, `cargo test -p audio-engine --test device -- --ignored`):
  - ✅ 3 entradas listadas: "Grupo de microfones (Realtek(R) Audio)" (padrão, 48 kHz, 2 canais), Steam Streaming Microphone e Oculus Virtual Audio Device;
  - ✅ captura de 2 s do padrão: 173 frames, `sample_index` monotônico, sem erro;
  - ✅ dispositivo inexistente dá erro claro;
  - ❓ o piso medido foi −104,8 dBFS, baixo demais para um microfone de notebook (provável microfone mudo ou bloqueado pela privacidade do Windows);
  - ⏳ validação com voz real (pitch, gate, clipping) depende do proprietário e entra no checklist manual.
- T-004-01 review → done (Agente 0, 2026-11-19): integrado em main @ 566a64e; fmt, clippy --workspace -D warnings e cargo test --workspace verdes (80 testes).
- Tarefas review → done (Agente 0, 2026-11-19): integrado em main @ af7bd2c; typecheck, vitest 105/105, vite build, clippy --workspace e cargo test --workspace verdes.
- ⏳ Microfone real: o Realtek padrão mediu −104,8 dBFS (provavelmente mudo ou bloqueado pela privacidade do Windows). Teste com voz pendente com o proprietário.
- T-004-04 `review` (Agente 1, 2026-11-19): `src-tauri/src/mic.rs`. `list_input_devices`; `start_mic_monitor` envia 1 de cada 3 frames (~33 Hz) pelo Channel, com `clipped` somado (OR) sobre os frames pulados; usa o `noiseFloorDbfs` salvo como gate; reiniciar substitui o medidor anterior. `stop_mic_monitor`; `calibrate_noise` aceita 1–10 s, roda `audio_engine::calibrate_noise` e grava `noiseFloorDbfs` nas settings. Erros de dispositivo viram `audio_device` com mensagem pt-BR. `MicFrame` é local até o `performance.rs` existir. ⏳ Sem teste automatizado com dispositivo (precisa de microfone); validado só por compilação e clippy.
- T-004-04 done (Agente 0, 2026-11-19): integrado em main; clippy --workspace e cargo test --workspace (92) verdes. Pendente: emitir `audio://error` (contrato novo).
- T-004-05 em review (Agente 3, 2026-11-19): `src/screens/Mic.tsx`.
  - R1: dispositivo persistido em `inputDeviceId`; "Padrão do sistema" = null; troca reinicia o monitor.
  - R2: `MicMeter` mostra nível (meter -60..0 dBFS), linha do noise floor, "Saturando", "Voz detectada" e a nota/Hz detectada.
  - R6: calibração de 3 s com `issues[]` em pt-BR, ou "Ambiente bom para cantar."
  - R7: ajuste manual de −500 a +500 ms em passos de 10 ms, persistido.
  - R9: aviso no Player (ver SPEC-003 STATUS).
  - Testes: `Mic.test.tsx` (5).
  - ⏳ O "teste de palmas sobre clique" do R7 não foi feito. Os frames do monitor chegam a ~30 Hz pelo Channel, resolução insuficiente para medir latência na UI sem enganar; precisa de um command que meça no Rust (proposta ao coordenador). Dispositivo real não testado.
- T-004-05 done (Agente 0, 2026-11-19): integrado em main; vitest 124/124. R7 assistido vira T-004-07.
- T-004-06, parte do Agente 2 `review` (2026-11-19): checagem de vazamento em `performance.rs`. `PerformanceConfig.leak_check` e `PerformanceSummary.leak: Option<LeakVerdict { passed, guide_score, reason }>` (`reason` em pt-BR quando reprova).
  - **Método:** com o usuário calado, a música toca e o Scorer roda normalmente. `guide_score` = componente P só nas notas da referência tocadas **inteiras** (as outras são excluídas com `Scorer::skip`).
  - **R9, limiar `passed`: `guide_score ≤ 0,05`**, e ainda: ≥ 8 s tocados, ≥ 3 notas cobertas, sem seek e com referência. Senão `passed = false` com motivo ("Teste curto demais…", "O teste foi interrompido…", "O teste precisa de uma música com referência…", "O microfone está captando a voz ou a melodia da música…"). A execução de checagem nunca é competitiva.
  - **Evidência** (`performance::tests::leak_threshold_evidence`, `--ignored`, ≈ 60 s): análise real, usuário calado, ruído de sala −60 dBFS, gate padrão −50 dBFS, música de 10 notas (12,5 s). "Score só com o vazamento" = nota que esse vazamento daria numa performance normal.

    | Fonte nas caixas | −20 | −30 | −40 | −45 | −50 | −55 dBFS | só ruído de sala |
    |---|---|---|---|---|---|---|---|
    | Voz-guia (vogal com formantes): guide_score / score | 0,998 / 99,7 | 0,998 / 99,7 | 0,998 / 99,7 | 0,998 / 99,8 | 0,590 / 59,2 | 0 / 0 | 0 / 0 |
    | Melodia instrumental (piano, decaimento 300 ms) | 1,000 / 100 | 0,990 / 99,2 | 0,650 / 72,0 | 0,479 / 57,1 | 0,302 / 34,4 | 0,131 / 14,3 | 0 / 0 |
    | Só acordes (fundamental e quinta uma oitava abaixo) | 0,891 / 89,2 | 0,891 / 89,2 | 0,891 / 89,2 | 0,891 / 89,2 | 0,878 / 88,2 | 0 / 0 | 0 / 0 |

  - **Leitura da evidência:**
    - O vazamento é quase binário: acima do gate do VAD ele dá a nota inteira; abaixo, nada.
    - Todos os casos aprovados têm `guide_score` 0, e o menor reprovado tem 0,131. O score do vazamento fica em ≈ 110·P, então 0,05 ≈ 5 pontos, que é o máximo que um dispositivo aprovado pode ganhar de graça (invariante do teste).
    - O acompanhamento sozinho também vaza quando a fundamental coincide com a melodia (89 pontos), então a checagem deve rodar com a música normal, com ou sem voz-guia.
  - Guarda rápido no CI: `leak_threshold_separates_a_silent_room_from_a_leak` e `leak_check_needs_8s_and_3_notes_and_no_seek`.
  - ❓ Com microfone físico e caixas reais ainda não foi medido. O gate depende da calibração de ruído: com o piso real mais alto, o vazamento precisa ser mais forte para passar do gate.
- T-004-07, parte do Agente 2 `review` (2026-11-19): sonda de latência ponta a ponta (R7) pelo mesmo caminho da performance, com o desenho aprovado pelo coordenador.
  - **audio-engine:** `detect_onsets` (envelope de 5 ms, subida ≥ 15 dB, ≥ piso + 20 dB, refratário de 150 ms, início com precisão de amostra), `latency_from` (pareamento em [−100, +600] ms, offset = mediana, jitter = desvio absoluto mediano) e `Capture::start_raw` (blocos mono numerados como o contador do callback).
  - **performance.rs:** `PerformanceConfig.latency_probe: Option<Vec<f64>>` (cliques em tempo de vídeo; exclusivo com `leak_check`; exige `latency_offset_ms = 0`, senão `invalid_input`) e `PerformanceSummary.latency: Option<LatencyVerdict { offsetMs, jitterMs, detected, expected, passed, reason }>`.
    - As amostras são guardadas a partir do 1º play, sem pré-roll.
    - Cada onset é mapeado com a âncora vigente na sua amostra (o histórico de âncoras inclui os resyncs).
    - Falha com motivo em pt-BR se houver seek, < 4 cliques tocados, detected < expected/2 ou |offset| > 500 ms.
  - Testes ✅:
    - puros: atraso constante de 87 ms medido com erro ≤ 1 ms; palmas com ±10 ms → jitter 10 ±1,5; palmas faltando; som contínuo não vira transiente; palma adiantada → offset negativo;
    - sessão: atraso medido através da âncora com o pré-roll ignorado; resync de +100 ms aplicado só aos onsets posteriores; sem detecção, com seek e sonda curta reprovam; validação e JSON achatado;
    - hardware (`--ignored`): `start_raw` com blocos contíguos no microfone Realtek; `live_probe_on_the_default_microphone` roda ponta a ponta (sem cliques tocados → detected 0 com motivo).
  - ⏳ Medida real com cliques nas caixas e palmas não foi feita. O microfone desta máquina parece mudo (piso de −105 a −109 dBFS); depende do proprietário.
- T-004-07 parte Agente 1 `review` (2026-11-19): `start_performance` com `latencyProbe` (1–500 cliques; recusa junto com leakCheck, rodada ou participante) monta `PerformanceConfig.latency_probe` com `latency_offset_ms = 0`, sem referência. No finish, nada vai para `performances`; se `passed`, grava `latencyOffsetMs = round(offsetMs)` pelo caminho validado das settings; devolve `outcome.latency` (achatado, como no ipc.ts). Teste ✅ `live::tests::latency_probe_saves_offset_only_when_passed`. ❓ `songId` continua obrigatório e precisa existir (a UI passa a música atual).
- T-004-07 (Agente 0, 2026-11-19): sonda integrada em main @ 2cef2b0. Sintético: atraso de 87 ms medido com erro ≤ 1 ms. ⏳ UI (trilha de cliques) e medição real com caixas e palmas.
- T-004-07, parte da UI (Agente 3, 2026-11-19), em review: "Medir automaticamente" na tela de Microfone (`src/screens/LatencyProbe.tsx`).
  - `src/player/clickTrack.ts` (puro) gera um WAV PCM 16-bit mono 48 kHz com 8 cliques de 6 ms (2 kHz, decaimento rápido), um a cada 750 ms, o 1º em 1000 ms; os ataques caem exatos na amostra.
  - O WAV toca por um `<audio>` (blob:, permitido na CSP `media-src`), o mesmo pipeline de saída dos vídeos. Passa `latencyProbe: clicksMs` (tempo de mídia) ao `startPerformance` e sincroniza com `currentTime` do elemento em `playing`, a cada 250 ms e no fim (`<audio>` não tem `requestVideoFrameCallback`).
  - No fim, mostra `outcome.latency`: se passou, "Latência medida: X ms (variação ±Y ms, D de E palmas). Ajuste salvo." e recarrega as settings; se não, o `reason` do Rust e "O ajuste atual foi mantido." O ajuste manual continua.
  - Testes: `clickTrack.test.ts` (3: espaçamento, WAV válido, ataques exatos) e `LatencyProbe.test.tsx` (4).
  - ❓ Usa a 1ª música disponível como `songId` (o command exige uma). ⏳ Medida real com caixas e palmas não feita.

