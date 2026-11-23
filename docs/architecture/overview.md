# Arquitetura — visão geral

Decisões de stack: [ADR-0001](../adr/ADR-0001-stack.md). Contratos entre módulos: [contracts.md](contracts.md).

## Processo único, três camadas

```
┌──────────────────────── WebView2 (React/TS) ───────────────────────┐
│  UI, navegação, <video> (relógio de mídia), feedback em tempo real │
│  apps/desktop/src  ──  commands próprios só via src/lib/ipc.ts     │
└───────────────▲────────────────────────────────┬───────────────────┘
                │ eventos/Channel                │ invoke()
┌───────────────┴────────────── Rust (Tauri) ────▼───────────────────┐
│ apps/desktop/src-tauri                                             │
│  commands.rs  db.rs (SQLite, migrações)  library.rs (scan, watch)  │
│  live.rs  mic.rs  reference.rs  sessions.rs  analysis.rs           │
│  settings.rs  performance.rs (ao vivo: relógio, sync, scorer)      │
│       │               │               │             │              │
│  audio-engine    scoring-engine   game         song-analysis       │
│  (cpal, YIN,     (referência,     (regras de   (separação ONNX,    │
│   VAD)            pontuação)       partida)     transcrição,       │
│                                                 alinhamento)       │
└────────────────────────────────────────────────────────────────────┘
```

Os quatro crates em `crates/` são **testáveis sem Tauri**. `scoring-engine` e `game` não fazem I/O. `audio-engine` só faz I/O de dispositivo de áudio. `song-analysis` lê o modelo ONNX do disco (sem rede) e usa os outros dois crates (orquestração da referência automática).

## Fluxo vertical prioritário

Selecionar pasta → indexar → buscar → reproduzir → escolher microfone → calibrar → cantar → pontuar contra referência → finalizar → salvar → ranking.

## Sincronização áudio ↔ vídeo (decisão central)

O `<video>` é o relógio da **música**; o contador de amostras do microfone é o relógio da **voz**. Nenhum dos dois sozinho basta.

1. Ao iniciar a captura, cada `AnalysisFrame` traz `sample_index` (contador monotônico do dispositivo).
2. A UI chama `sync_performance(videoTimeS, playing, seeked)` em `play`, `pause`, `seeking/seeked` e a cada ~250 ms via `requestVideoFrameCallback` (usa `mediaTime`, não o relógio da UI).
3. `performance.rs` mantém uma âncora `(video_time, sample_index_at_receipt)`. Tempo de música de um frame:
   `t_song = anchor_video + (sample_index - anchor_sample)/sample_rate - latency_offset`
4. Resync periódico corrige deriva: se |estimado − reportado| > 40 ms, re-ancora (sem reprocessar frames antigos).
5. **Pausa**: frames descartados enquanto `playing=false`. **Retomar**: nova âncora.
6. **Seek** em performance competitiva: a performance vira **treino** (não entra no ranking). Em treino, notas puladas são ignoradas.
7. `latency_offset` = latência de entrada do microfone + latência de saída do vídeo, medida no assistente de calibração (SPEC-004) ou ajustada manualmente.

## Referência melódica

Sem referência não há pontuação competitiva (SPEC-005). Ordem de preferência ([ADR-0002](../adr/ADR-0002-melody-reference.md)): importação MIDI/JSON/UltraStar → gravação cantada → edição/sincronização manual → extração automática a partir do vídeo (SPEC-012, experimental, modelo opcional baixado pelo usuário). Estados: `none` (sem linha) · `processing` · `experimental` · `validated` · `invalid`.

## Persistência

SQLite em `app_data_dir()/masterofnotes.db`, acesso **só pelo Rust** (`rusqlite`, bundled). Migrações SQL embutidas, aplicadas por `PRAGMA user_version`. Backup automático do arquivo antes de qualquer migração (Lei 14). Esquema: [contracts.md §3](contracts.md#3-esquema-sqlite-migrações-001-e-002).

## Vídeo

`<video>` nativo do WebView2 via asset protocol (`convertFileSrc`). Escopo do asset protocol liberado em runtime **arquivo a arquivo**, só para os vídeos indexados e disponíveis (nunca a pasta inteira). Codec incompatível → estado `Error` com mensagem clara e `songs.playable='no'`. Sem transcodificação no MVP ([ADR-0003](../adr/ADR-0003-video-playback.md)).

## Atualização

`tauri-plugin-updater` + manifesto `latest.json` em GitHub Releases, assinado (minisign). Chave privada fora do repositório (secret do CI). Fluxo com consentimento explícito (SPEC-009).

## Estrutura do repositório

```
apps/desktop/            React + Vite + Tailwind (Agente 3)
apps/desktop/src-tauri/  camada nativa (Agente 1; performance.rs = Agente 2)
crates/audio-engine/     captura + DSP (Agente 2)
crates/scoring-engine/   referência + pontuação (Agente 2)
crates/game/             regras de jogo (Agente 4)
crates/song-analysis/    referência automática: separação, transcrição, alinhamento (Agente 2)
specs/                   SPEC-NNN.md (spec+plan+tasks+test+status num arquivo)
docs/adr/                decisões
docs/architecture/       este documento + contratos
docs/testing/            estratégia e evidências
tests/fixtures/          fixtures versionadas (referências; o áudio sintético é gerado nos testes)
```

Não criados (ainda): `crates/core`, `packages/ui`. Serão criados quando houver segundo consumidor (Lei 06).
