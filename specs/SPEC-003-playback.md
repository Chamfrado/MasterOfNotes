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
| T-003-01 | Reducer da máquina de estados + testes | Agente 3 | todo |
| T-003-02 | Componente Player + atalhos + tela cheia | Agente 3 | todo |
| T-003-03 | Integração com `syncPerformance` | Agente 3 | todo |
| T-003-04 | Checklist manual de formatos (mp4/webm/mov/mkv/avi) no WebView2 | Agente 4 | todo |

## TEST
`playerMachine.test.ts` (todas as transições válidas e inválidas); checklist em `docs/testing/evidence/codecs.md`.

## STATUS
Pendente.
