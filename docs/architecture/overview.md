# Arquitetura — visão geral

Decisões de stack: [ADR-0001](../adr/ADR-0001-stack.md). Contratos entre módulos: [contracts.md](contracts.md).

## Processo único, três camadas

```
┌──────────────────────── WebView2 (React/TS) ───────────────────────┐
│  UI, navegação, <video> (relógio de mídia), feedback em tempo real │
│  apps/desktop/src  ──  único ponto de IPC: src/lib/ipc.ts          │
└───────────────▲────────────────────────────────┬───────────────────┘
                │ eventos/Channel                │ invoke()
┌───────────────┴────────────── Rust (Tauri) ────▼───────────────────┐
│ apps/desktop/src-tauri                                             │
│  commands/*  db/* (SQLite, migrações)  library/* (scan, watch)     │
│  performance.rs (sessão ao vivo: relógio, sync, scorer)            │
│        │                    │                     │                │
│  crates/audio-engine  crates/scoring-engine   crates/game          │
│  (cpal, YIN, VAD)     (referência, score)     (regras de partida)  │
└────────────────────────────────────────────────────────────────────┘
```

Os três crates em `crates/` são **puros e testáveis sem Tauri**. `scoring-engine` e `game` não fazem I/O. `audio-engine` só faz I/O de dispositivo de áudio.

## Fluxo vertical prioritário

Selecionar pasta → indexar → buscar → reproduzir → escolher microfone → calibrar → cantar → pontuar contra referência → finalizar → salvar → ranking.

## Sincronização áudio ↔ vídeo (decisão central)

O `<video>` é o relógio da **música**; o contador de amostras do microfone é o relógio da **voz**. Nenhum dos dois sozinho basta.

1. Ao iniciar a captura, cada `AnalysisFrame` traz `sample_index` (contador monotônico do dispositivo).
2. A UI chama `sync_performance(videoTimeS, playing)` em `play`, `pause`, `seeking/seeked` e a cada ~250 ms via `requestVideoFrameCallback` (usa `mediaTime`, não o relógio da UI).
3. `performance.rs` mantém uma âncora `(video_time, sample_index_at_receipt)`. Tempo de música de um frame:
   `t_song = anchor_video + (sample_index - anchor_sample)/sample_rate - latency_offset`
4. Resync periódico corrige deriva: se |estimado − reportado| > 40 ms, re-ancora (sem reprocessar frames antigos).
5. **Pausa**: frames descartados enquanto `playing=false`. **Retomar**: nova âncora.
6. **Seek** em performance competitiva: a performance vira **treino** (não entra no ranking). Em treino, notas puladas são ignoradas.
7. `latency_offset` = latência de entrada do microfone + latência de saída do vídeo, medida no assistente de calibração (SPEC-004) ou ajustada manualmente.

## Referência melódica

Sem referência não há pontuação competitiva (SPEC-005). Ordem de preferência ([ADR-0002](../adr/ADR-0002-melody-reference.md)): importação MIDI/JSON → edição/sincronização manual → extração automática (experimental, fase posterior). Estados: `none` (sem linha) · `processing` · `experimental` · `validated` · `invalid`.

## Persistência

SQLite em `app_data_dir()/masterofnotes.db`, acesso **só pelo Rust** (`rusqlite`, bundled). Migrações SQL embutidas, aplicadas por `PRAGMA user_version`. Backup automático do arquivo antes de qualquer migração (Lei 14). Esquema: [contracts.md §3](contracts.md#3-esquema-sqlite-migração-001).

## Vídeo

`<video>` nativo do WebView2 via asset protocol (`convertFileSrc`). Escopo do asset protocol liberado em runtime **apenas** para a pasta da biblioteca. Codec incompatível → estado `Error` com mensagem clara e `songs.playable='no'`. Sem transcodificação no MVP ([ADR-0003](../adr/ADR-0003-video-playback.md)).

## Atualização

`tauri-plugin-updater` + manifesto `latest.json` em GitHub Releases, assinado (minisign). Chave privada fora do repositório (secret do CI). Fluxo com consentimento explícito (SPEC-009).

## Estrutura do repositório

```
apps/desktop/            React + Vite + Tailwind (Agente 3)
apps/desktop/src-tauri/  camada nativa (Agente 1; performance.rs = Agente 2)
crates/audio-engine/     captura + DSP (Agente 2)
crates/scoring-engine/   referência + pontuação (Agente 2)
crates/game/             regras de jogo (Agente 4)
specs/                   SPEC-NNN.md (spec+plan+tasks+test+status num arquivo)
docs/adr/                decisões
docs/architecture/       este documento + contratos
docs/testing/            estratégia e evidências
tests/fixtures/          fixtures versionadas (áudio sintético, referências)
```

Não criados (ainda): `crates/core`, `crates/song-analysis`, `packages/ui`. Serão criados quando houver segundo consumidor ou extração automática de melodia (Lei 06).
