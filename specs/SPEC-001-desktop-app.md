# SPEC-001 — Aplicação desktop, configurações e persistência

## SPEC
- R1. Janela 1280×800 (mín. 960×600), redimensionável, tela cheia (F11).
- R2. Configurações (`AppSettings` em `ipc.ts`) persistem em SQLite e sobrevivem a reinício. Valores validados no Rust (faixas, enums). `libraryFolder` só muda via `set_library_folder` (`update_settings` recusa com `invalid_setting`). `autoCheckUpdates` começa `false`; a UI pede consentimento no primeiro uso (Lei 10).
- R3. Banco em `app_data_dir()/masterofnotes.db`; migrações aplicadas na inicialização por `PRAGMA user_version`; backup `masterofnotes.db.bak-v{N}` antes de migrar banco existente; `foreign_keys=ON`, `journal_mode=WAL`.
- R4. Falha ao abrir/migrar banco: tela de erro com o caminho do arquivo; nada é apagado.
- R5. Logs em `app_log_dir()` (tauri-plugin-log), sem áudio e sem dados pessoais desnecessários.

Critérios:
- Given configurações alteradas, When o app fecha e reabre, Then `get_settings` retorna os valores alterados.
- Given banco na versão N com dados, When migração N+1 roda, Then os dados permanecem, existe backup e `user_version = N+1`.
- Given `update_settings({videoVolume: 7})`, Then erro `invalid_setting` e nada é gravado.

## PLAN
`rusqlite` (bundled). `db::open(path)` aplica `migrations/NNN_*.sql` (embutidas com `include_str!`). `Mutex<Connection>` em `tauri::State`. Settings: uma linha JSON por chave; defaults no Rust.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-001-01 | Workspace, scaffold Tauri, crates vazios | Agente 0 | done |
| T-001-02 | `db` + migração 001 + backup + testes | Agente 1 | todo |
| T-001-03 | `get_settings`/`update_settings` + validação | Agente 1 | todo |
| T-001-04 | `AppError` serializável + tauri-plugin-log | Agente 1 | todo |
| T-001-05 | Janela, ícone, CSP, capabilities mínimas | Agente 1 | todo |
| T-001-06 | CI (`.github/workflows/ci.yml`) + verificação da Lei 03 | Agente 4 | done |

## TEST
`db::tests::{migrates_fresh, reopen_preserves_data, backup_before_migration}`, `settings::tests::{roundtrip, rejects_invalid}`.

## STATUS
Scaffold criado e `cargo check --workspace` verde (2026-11-19). Restante pendente.
- T-001-06 em review (Agente 4, 2026-11-19): `.github/workflows/ci.yml` (windows-latest). Local ✅: grep Lei 03 (limpo + casos plantados), pnpm install/typecheck/test/build, cargo fmt --check, cargo test/clippy -p game. Só no CI ⏳: clippy/test --workspace.
- T-001-06 done (Agente 0, 2026-11-19): em main, os mesmos passos do CI rodaram localmente e passaram: build do front, fmt, clippy --workspace -D warnings e cargo test --workspace (game 15/15). Ainda não executado no GitHub (sem push) ⏳.
