# SPEC-013 — Importar referência UltraStar (.txt)

Decisão: [ADR-0002](../docs/adr/ADR-0002-melody-reference.md) (UltraStar como segunda importação). Versão-alvo: 0.1.4.

## SPEC
- R1. **Importar `.txt` UltraStar** pela mesma ação de importação da tela Referência (MIDI, JSON e agora UltraStar). Cabeçalho: `#BPM` (obrigatório), `#GAP` (ms, padrão 0), `#TITLE`/`#ARTIST` (informativos), `#RELATIVE` (suportar `yes`), `#ENCODING` ou detecção UTF-8/Latin-1 (arquivos antigos são CP1252).
- R2. **Notas:** `:` normal e `*` dourada viram notas; `F` (freestyle) e `R`/`G` (rap) **ficam fora** (não têm altura avaliável); `-` (quebra de linha) e `E` (fim) são estrutura. Tempo: `início_ms = GAP + batida × 60000 / (BPM × 4)`; duração idem. Altura UltraStar 0 = Dó4 = MIDI 60 (`midi = 60 + pitch`). A sílaba vira `lyric`.
- R3. **Dueto** (`P1`/`P2`): importa só o `P1` e avisa: "Arquivo de dueto: importamos a parte do primeiro cantor."
- R4. **Validação** igual à das outras fontes (notas ordenadas, sem sobreposição, MIDI de 24 a 108, ≥ 1 nota); erro em pt-BR com o número da linha quando a linha for inválida.
- R5. **Fonte:** nova `source = 'ultrastar'` (selo "UltraStar"). Isso exige uma **migração 002** (o CHECK da coluna `source` muda), com backup antes, testada com um banco da 0.1.3 com dados (Lei 14).
- R6. **Alinhamento:** o UltraStar marca tempo em relação ao áudio original (#MP3), não ao vídeo de karaokê. Depois de importar, a tela sugere **Alinhar automaticamente** (0.1.3).
- R7. **Licença (Lei 19):** o app não inclui nem baixa arquivos UltraStar; o guia orienta a usar só arquivos que a pessoa tem direito de usar.

Critérios:
- Given um `.txt` com `#BPM:300`, `#GAP:1000` e `: 0 4 0 Lá`, Then nota em 1000 ms, duração 200 ms, MIDI 60, lyric "Lá".
- Given linhas `F` e `R`, Then não viram notas.
- Given dueto, Then só P1 + aviso.
- Given um banco da 0.1.3 com referências e performances, When o app 0.1.4 abre, Then a migração 002 preserva tudo e existe backup `.bak-v1`.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-013-01 | `scoring-engine`: `import_ultrastar(bytes) -> Result<(MelodyReference, UltraStarInfo), RefError>` + testes (BPM/GAP, relativo, F/R/G, dueto, encoding, linha inválida) | Agente 2 | review |
| T-013-02 | Migração 002 (`source` com `'ultrastar'`) + teste de migração com dados da 0.1.3; `import_reference` aceita `.txt` | Agente 1 | review |
| T-013-03 | UI: filtro `.txt` no diálogo, selo "UltraStar", aviso de dueto, sugestão de alinhar | Agente 3 | review |
| T-013-04 | fake-backend + E2E + QA (arquivos UltraStar reais de autoria própria, em `tests/fixtures/references/`) | Agente 4 | todo |
| T-013-05 | Guia (05, FAQ): o que é UltraStar, como importar, por que alinhar, aviso de direitos | Agente 5 | todo |

## STATUS
Criada em 2026-11-20 (coordenador), janela autônoma.
- T-013-03 `review` (Agente 3, 2026-11-20):
  - Selo "UltraStar". O diálogo de importação aceita `.txt`, com o filtro "MIDI, JSON ou UltraStar (.txt)", e o texto de "Sem referência" cita UltraStar.
  - Se `importReference` devolve `notice` (ex.: dueto, R3), ele aparece em toast.
  - R6: depois de importar um UltraStar, um aviso (`role="status"`) sugere "Alinhar automaticamente". Ele some quando o alinhamento é pedido ou quando outra referência substitui a importada.
  - Testes: `Reference.test.tsx` +1 (filtro, selo, dueto, sugestão até alinhar). ⏳ Não visto com o `import_ultrastar` real (T-013-01/02).
- 2026-11-20 · Agente 2 · T-013-01 em `review`. Novo `crates/scoring-engine/src/ultrastar.rs`; `import_ultrastar` e `UltraStarInfo` (com `Serialize` em camelCase) são exportados.
  - **Formato:**
    - `#BPM` é obrigatório e aceita vírgula decimal;
    - `#GAP` vale 0 se faltar;
    - `#RELATIVE:yes`, com quebra de linha de 1 ou 2 números (soma o 2º, ou o único);
    - `:`/`*` viram notas; `F`/`R`/`G` ficam fora; `-`/`E` são estrutura, e o que vem depois de `E` é ignorado;
    - `P1`/`P3` entram, `P2` fica fora, e `duet` vale `true` se houver `P2`;
    - a sílaba é aparada, e `~` de continuação vira `lyric` vazia;
    - UTF-8 com ou sem BOM; CP1252 se os bytes não forem UTF-8 válido ou se `#ENCODING` declarar CP1252/Latin-1.
  - **Erros (contrato, a aprovar):** duas variantes novas de `RefError`, com Display em pt-BR:
    - `UltraStarHeader(String)` → "arquivo UltraStar inválido: falta o #BPM";
    - `UltraStarLine { line, reason }` → "arquivo UltraStar, linha N: …".
  - **Validação e skyline:** as mesmas das outras fontes (`validate`).
  - **Fixtures próprias:** `ultrastar-simple.txt`, `ultrastar-cp1252-relative.txt` e `ultrastar-duet.txt`. O CRLF é testado inline, porque o repo normaliza as fixtures para LF.
  - **Testes:**
    - `cargo test -p scoring-engine` ✅ (lib 32, reference 16 com 7 novos, regression 33);
    - clippy `-D warnings` ✅;
    - `ALGORITHM_VERSION` não mudou.
- T-013-02 em `review` (Agente 1, 2026-11-20).
  - **Migração 002** (`src-tauri/migrations/002_ultrastar_source.sql`): recria `melody_references` pelo procedimento oficial do SQLite (CREATE nova + INSERT SELECT com os ids + DROP + RENAME + índice parcial `one_active_reference` recriado), para o CHECK de `source` aceitar `'ultrastar'`.
  - O runner (`db.rs`) desliga `foreign_keys` antes das migrações, porque o PRAGMA não muda dentro de transação, e religa depois. Cada migração roda `pragma_foreign_key_check` antes do commit: qualquer violação vira `db_migration_failed` em pt-BR, com rollback, e nada é alterado. O backup `.bak-v1` continua automático (`VACUUM INTO`).
  - **Teste de migração** (portão 6): `db::tests::migration_002_keeps_013_data_and_accepts_ultrastar`. O banco é montado só com a migração 001, o formato exato da 0.1.3, com 15 linhas: settings, 2 músicas (uma indisponível), referências ativa + inativa + extracted com offset negativo, participantes, sessão party, rodadas e 3 performances apontando para as referências 1, 2 e 5.
    - Ao abrir com o app novo: `user_version` 2; dump de TODAS as tabelas idêntico, ids incluídos; `foreign_key_check` vazio; FKs religadas; `.bak-v1` com versão 1 e os mesmos dados.
    - O novo esquema aceita 'ultrastar', recusa fonte desconhecida, mantém o índice parcial e as FKs (para `songs` e a de `performances.reference_id` para a tabela recriada). Reabrir um banco v2 não gera backup.
    - `db::tests::migration_with_dangling_reference_rolls_back`: uma migração que deixa referência pendurada é desfeita e o banco fica em v1, idêntico.
  - **Import UltraStar:** `import_reference` aceita `.txt` → `scoring_engine::import_ultrastar` (Agente 2, agent-2/work @ 220b8f6) → `source = 'ultrastar'`, experimental, ativa, pelo mesmo `store()`.
    - Dueto: `MelodyReference.notice` = "Arquivo de dueto: importamos a parte do primeiro cantor." (opção A aprovada). Só o retorno do import tem o campo; fora isso, ele não aparece no JSON.
    - Arquivo inválido → `invalid_reference` com "linha N" na mensagem.
  - Testes ✅ `reference::tests::import_ultrastar_txt_with_duet_notice` (critério 300 BPM/GAP 1000 → 1000 ms/200 ms/MIDI 60/"Lá"; F fora; dueto só P1 + aviso; notice omitido; "linha 2") e `rejects_bad_files_and_saves_nothing` (.txt que não é UltraStar → `invalid_reference`). `cargo test -p master-of-notes`: 71 ok, 7 ignorados. Clippy `-D warnings` limpo.
