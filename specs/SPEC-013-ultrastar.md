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
| T-013-02 | Migração 002 (`source` com `'ultrastar'`) + teste de migração com dados da 0.1.3; `import_reference` aceita `.txt` | Agente 1 | todo |
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
