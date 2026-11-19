# SPEC-003 — Reprodução de vídeo

Decisão: [ADR-0003](../docs/adr/ADR-0003-video-playback.md). Sincronização: [overview.md](../docs/architecture/overview.md#sincronização-áudio--vídeo-decisão-central).

## SPEC
- R1. Máquina de estados: `idle → loading → ready → countdown → playing ⇄ paused → completed`; qualquer estado → `error`. Transições inválidas são ignoradas (e testadas).
- R2. Controles: play/pause (Espaço), tela cheia (F), volume (↑/↓, persistido), sair (Esc), barra de progresso. Seek (←/→ e barra) só em modo treino; em competitivo o seek converte a performance em treino com aviso.
- R3. Erro de codec: mensagem clara (formato, sugestão de converter para MP4 H.264/AAC) e `playable='no'`.
- R4. `loadedmetadata` grava `durationMs` e `playable='yes'`.
- R5. Contagem regressiva 3-2-1 antes do play em partidas.
- R6. Chama `syncPerformance` em play/pause/seeking/seeked e a cada ~250 ms via `requestVideoFrameCallback` (`metadata.mediaTime`). O relógio da UI nunca é a fonte do tempo de música.
- R7. Overlay de feedback não cobre o terço inferior central (letras); modos `full`/`compact`/`hidden` (tecla H).
- R8. Pausar suspende a captura de pontuação (via `syncPerformance(playing=false)`); retomar re-ancora.

## PLAN
`playerMachine.ts`: reducer puro (estado + evento → estado), testável sem DOM. Componente `Player` consome o reducer.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-003-01 | Reducer da máquina de estados + testes | Agente 3 | done |
| T-003-02 | Componente Player + atalhos + tela cheia | Agente 3 | review |
| T-003-03 | Integração com `syncPerformance` | Agente 3 | review |
| T-003-04 | Checklist manual de formatos (mp4/webm/mov/mkv/avi) no WebView2 | Agente 4 | doing |

## TEST
`playerMachine.test.ts` (todas as transições válidas e inválidas); checklist em `docs/testing/evidence/codecs.md`.

## STATUS
- T-003-04 em andamento (Agente 4, 2026-11-19): checklist e roteiro de fixtures sintéticas prontos em `docs/testing/evidence/codecs.md`; execução ⏳ aguarda o Player (T-003-02).
- T-003-01 em review (Agente 3, 2026-11-19): `src/player/playerMachine.ts`; `playerMachine.test.ts` cobre a tabela completa estado × evento (8 × 9: válidas checam o estado resultante, inválidas checam identidade do objeto) + contagem 3-2-1 + fluxo completo.
- T-003-02/03 em review (Agente 3, 2026-11-19): `src/screens/Player.tsx`.
  - Controles: Espaço (pausar/continuar), F (tela cheia da janela Tauri, desfeita ao sair), H (overlay full → compact → hidden, persistido em `feedbackOverlay`), ↑/↓ (volume em passos de 5%, persistido em `videoVolume`), ←/→ (±5 s), barra de posição, Esc (sair; a desmontagem chama `abortPerformance`). Os controles somem após 3 s cantando e voltam com mouse ou tecla.
  - R2 seek: em partida competitiva, um diálogo avisa que a partida vira treino. Ao confirmar, aparece o selo "Treino — fora do ranking" e os próximos seeks não perguntam mais. A conversão em si é do Rust (`seeked=true`).
  - R6/R8: `syncPerformance` em play (playing=true), pause (false), seeking (false, seeked) e seeked (estado real, seeked), mais `requestVideoFrameCallback` com `mediaTime` a cada 250 ms durante playing.
  - SPEC-004 R9 no player: `src/player/leak.ts` avisa "o microfone está captando a música — use fones" se mais de 20% dos últimos 300 frames fora de notas (`feedback == null`) tiverem voz.
  - Testes: `Player.test.tsx` (9, jsdom com mídia simulada) e `leak.test.ts` (4).
  - ⏳ Não executado no WebView2 real: `requestVideoFrameCallback`, tela cheia, codecs (T-003-04 do Agente 4 pode rodar agora).
- Demais pendentes.
- Tarefas review → done (Agente 0, 2026-11-19): integrado em main @ af7bd2c; typecheck, vitest 105/105, vite build, clippy --workspace e cargo test --workspace verdes.
