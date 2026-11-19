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
| T-004-04 | Commands `list_input_devices`, `start/stop_mic_monitor`, `calibrate_noise` | Agente 1 | review |
| T-004-05 | Tela de microfone e calibração | Agente 3 | review |

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
- T-004-05 em review (Agente 3, 2026-11-19): `src/screens/Mic.tsx`.
  - R1: dispositivo persistido em `inputDeviceId`; "Padrão do sistema" = null; troca reinicia o monitor.
  - R2: `MicMeter` mostra nível (meter -60..0 dBFS), linha do noise floor, "Saturando", "Voz detectada" e a nota/Hz detectada.
  - R6: calibração de 3 s com `issues[]` em pt-BR, ou "Ambiente bom para cantar."
  - R7: ajuste manual de −500 a +500 ms em passos de 10 ms, persistido.
  - R9: aviso no Player (ver SPEC-003 STATUS).
  - Testes: `Mic.test.tsx` (5).
  - ⏳ O "teste de palmas sobre clique" do R7 não foi feito. Os frames do monitor chegam a ~30 Hz pelo Channel, resolução insuficiente para medir latência na UI sem enganar; precisa de um command que meça no Rust (proposta ao coordenador). Dispositivo real não testado.
