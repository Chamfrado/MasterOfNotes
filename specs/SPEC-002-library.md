# SPEC-002 — Catálogo local de músicas

## SPEC
- R1. Usuário escolhe uma pasta (diálogo nativo). Varredura recursiva configurável (padrão: sim).
- R2. Extensões: mp4, mkv, webm, mov, avi (sem distinção de caixa). Outros arquivos ignorados.
- R3. Vídeos **não** são copiados; o banco guarda o caminho.
- R4. ID estável = blake3(tamanho ‖ primeiros 64 KiB ‖ últimos 64 KiB), hex. Renomear/mover dentro da pasta preserva o ID e o histórico. Mesmo conteúdo em dois caminhos: um registro (último caminho visto), duplicata registrada no log.
- R5. Arquivo inalterado (mesmo caminho, tamanho e mtime) não é re-hasheado.
- R6. Arquivo removido → `available = 0` (histórico preservado), oculto da biblioteca.
- R7. Watcher detecta criação/remoção/alteração/renomeação e emite `library://changed` sem reiniciar (debounce ≤ 2 s).
- R8. Título inicial = nome do arquivo sem extensão; editável (`update_song`). Renomear arquivo atualiza o título só se ele nunca foi editado.
- R9. Varredura fora da thread principal; UI responsiva.
- R10. Busca por título/artista/nome do arquivo, sem distinção de caixa e acentos (na UI, sobre `list_songs`).
- R11. Ao abrir o app, o catálogo salvo aparece imediatamente; a re-varredura roda em segundo plano.

Critérios:
- Given pasta com 3 `.mp4` e 1 `.txt`, When seleciono a pasta, Then 3 músicas aparecem.
- Given música indexada, When renomeio o arquivo, Then o mesmo `id` aparece com o novo caminho.
- Given arquivo apagado, When o watcher dispara, Then a música some da lista e suas performances continuam no banco.

## PLAN
`library::scan(root, recursive, &db)` com `std::fs` recursivo; hashing com pool limitado; `notify-debouncer-full` para o watcher. Asset scope: `app.asset_protocol_scope().allow_directory(root, recursive)` ao definir a pasta e na inicialização.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-002-01 | Scanner + content id + upsert + testes com tempdir | Agente 1 | done |
| T-002-02 | Commands `set_library_folder`, `rescan_library`, `list_songs`, `update_song` | Agente 1 | done |
| T-002-03 | Watcher + evento `library://changed` | Agente 1 | done |
| T-002-04 | Tela Biblioteca: grade/lista, busca, estados vazio/loading/erro | Agente 3 | todo |

## TEST
`library::tests::{indexes_supported_only, stable_id_on_rename, skips_unchanged, marks_removed}`; `Library.test.tsx` (busca sem acento, estado vazio).

## STATUS
2026-11-19 (Agente 1): T-002-01..03 em review. ID = 32 primeiros hex do blake3. Varredura e hashing em `spawn_blocking`, sem segurar o lock do banco durante o hashing; uma varredura por vez. Regras extras: troca de nomes entre arquivos preserva os IDs; conteúdo alterado no mesmo caminho troca o ID só se a música não tem histórico (senão mantém o antigo); linha sumida cujo caminho foi ocupado por outro conteúdo fica com `path#id`. Watcher: debounce 1,5 s, qualquer evento (exceto Access) dispara re-varredura incremental e emite `library://changed` se houver mudança. Testes: `library::tests::{indexes_supported_only, stable_id_on_rename, swap_names_keeps_ids, skips_unchanged, marks_removed, duplicate_content_single_record, content_changed_in_place, content_id_uses_head_and_tail, update_song_validates}` verdes. Pendente: watcher, `set_library_folder` e asset protocol verificados só por compilação; validar no app (UI do Agente 3) ou em e2e (Agente 4). Plugin JS `@tauri-apps/plugin-dialog` a cargo do Agente 3.
- Tarefas done (Agente 0, 2026-11-19): integrado em main @ 566a64e; fmt, clippy --workspace -D warnings e cargo test --workspace verdes (80 testes). Ressalvas ⏳: watcher, asset protocol e CSP só verificados por compilação, falta validar com a UI real. Onda 2: restringir o escopo do asset protocol às extensões de vídeo (Lei 09).
