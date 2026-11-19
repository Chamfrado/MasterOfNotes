# ADR-0003 — Reprodução de vídeo

- **Status:** Aceita · 2026-11-19 · Agente 0

## Decisão

Elemento `<video>` do WebView2 com arquivos servidos pelo asset protocol do Tauri, escopo liberado em runtime só para a pasta da biblioteca.

Suporte esperado no WebView2 (Chromium, Windows) — **a confirmar com teste real por formato** (SPEC-003):

| Extensão | Expectativa |
|---|---|
| MP4 (H.264/AAC) | Suportado |
| WEBM (VP8/VP9/AV1, Opus/Vorbis) | Suportado |
| MOV (H.264/AAC) | Geralmente reproduz (container ISO BMFF) |
| MKV | Parcial: costuma funcionar com VP8/VP9/Opus; H.264+AAC/AC3 não garantido |
| AVI | Não suportado pelo Chromium |

Detecção: evento `error` / `MEDIA_ERR_SRC_NOT_SUPPORTED` → `songs.playable = 'no'` e mensagem explicando o codec e sugerindo conversão para MP4 (H.264/AAC). AVI aparece no catálogo marcado como "formato não suportado" para o usuário entender por que não toca.

## Alternativas

- **Transcodificação com FFmpeg embutido:** resolveria MKV/AVI, mas adiciona ~80 MB, tem implicações LGPL/GPL e patentes (H.264/AAC). Adiado; reavaliar após MVP com ADR própria.
- **Player nativo (mpv/libVLC) em janela filha:** sincronização e sobreposição de UI complexas, licenças LGPL/GPL. Rejeitado para o MVP.
