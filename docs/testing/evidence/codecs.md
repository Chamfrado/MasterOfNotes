# Evidência — formatos de vídeo no WebView2 (T-003-04)

Checklist manual de [SPEC-003](../../../specs/SPEC-003-playback.md) (R3, R4). **Ainda não executado**: depende do Player (T-003-02). Nada aqui conta como testado até a tabela de resultados estar preenchida e datada (Lei 17).

## Ambiente (preencher a cada execução)

| Campo | Valor |
|---|---|
| Data | |
| Executor | |
| Commit do app | `git rev-parse --short HEAD` |
| Windows | `winver` |
| WebView2 Runtime | Configurações › Aplicativos › "Microsoft Edge WebView2 Runtime" (versão) |
| Extensão HEVC instalada? | sim / não |
| Build | `pnpm dev` / instalador |

## Fixtures (sintéticas, Lei 19)

Geradas localmente com ffmpeg (barras de teste + senoide de 440 Hz, 10 s). **Não versionar** os vídeos; só este roteiro.

```sh
mkdir -p codec-fixtures && cd codec-fixtures
SRC="-f lavfi -i testsrc2=size=1280x720:rate=30:duration=10 -f lavfi -i sine=frequency=440:duration=10"
ffmpeg $SRC -c:v libx264 -pix_fmt yuv420p -c:a aac        h264_aac.mp4
ffmpeg $SRC -c:v libx265 -pix_fmt yuv420p -tag:v hvc1 -c:a aac hevc_aac.mp4
ffmpeg $SRC -c:v libvpx-vp9 -c:a libopus                  vp9_opus.webm
ffmpeg $SRC -c:v libx264 -pix_fmt yuv420p -c:a aac        h264_aac.mov
ffmpeg $SRC -c:v libx264 -pix_fmt yuv420p -c:a aac        h264_aac.mkv
ffmpeg $SRC -c:v libvpx-vp9 -c:a libopus                  vp9_opus.mkv
ffmpeg $SRC -c:v mpeg4 -c:a libmp3lame                    mpeg4_mp3.avi
```

Também copiar um arquivo corrompido: `head -c 50000 h264_aac.mp4 > truncated.mp4`.

## Procedimento por arquivo

1. Adicionar a pasta `codec-fixtures` à biblioteca e abrir o arquivo no Player.
2. **Toca?** Imagem e áudio (440 Hz) presentes, sem travar por 10 s.
3. **R4**: após `loadedmetadata`, a música mostra duração ≈ 10 s e `playable='yes'`.
4. **R3**: se não tocar, a mensagem cita o formato e sugere converter para MP4 H.264/AAC; a música fica `playable='no'` e o app continua utilizável.
5. Pausar/retomar e (modo treino) seek ←/→ funcionam.

Para ver `playable`/`duration_ms`: `sqlite3 <banco SQLite do app, caminho definido em T-001-02> "select file_name, playable, duration_ms from songs;"`.

## Resultados

Coluna "Esperado" é hipótese (❓) baseada no suporte de codecs do Chromium/Edge; o resultado real é o que vale.

| Arquivo | Esperado ❓ | Toca | Áudio | Duração/`playable` (R4) | Mensagem de erro (R3) | Pausa/seek | Observações |
|---|---|---|---|---|---|---|---|
| h264_aac.mp4 | toca | | | | n/a | | |
| hevc_aac.mp4 | só com extensão HEVC | | | | | | |
| vp9_opus.webm | toca | | | | n/a | | |
| h264_aac.mov | provável | | | | | | |
| h264_aac.mkv | incerto | | | | | | |
| vp9_opus.mkv | provável | | | | | | |
| mpeg4_mp3.avi | não toca → R3 | | | | | | |
| truncated.mp4 | erro → R3 | | | | | | |

Conclusão: _(formatos suportados oficialmente; texto de ajuda da UI ajustado?)_
