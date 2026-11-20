# QA independente do UltraStar e da migração 002 (SPEC-013, T-013-04)

Agente 4 · 2026-11-20

## Base
- **Parser:** `scoring_engine::import_ultrastar` @ agent-2/work 92fb1ad.
- **Migração e import:** agent-1/work @ 49d15ca.
- Os dois foram trazidos por merge local na agent-4/work.

**Material:**
- 25 arquivos UltraStar de autoria própria (Lei 19) em `tests/fixtures/references/ultrastar/`, com o README listando os valores esperados.
- O gerador está em [ultrastar-fixtures-gen.py](ultrastar-fixtures-gen.py).
- O `.gitattributes` da pasta (`*.txt -text`) mantém os bytes exatos: a regra `text=auto eol=lf` da raiz tinha normalizado o `crlf.txt`.

**Sondas:**
- [ultrastar-qa-probes.rs](ultrastar-qa-probes.rs): fixtures + fuzz leve;
- [migration-002-qa.py](migration-002-qa.py): Python só com stdlib, independente do código Rust.

## Veredito
**Nenhum achado ALTO nem MÉDIO.** O parser e a migração 002 estão aptos para a 0.1.4. Um achado baixo (B1, já corrigido) e duas observações.

| ID | Gravidade | Achado | Sugestão |
|---|---|---|---|
| B1 | Baixo, ✅ **corrigido** (agent-2/work @ d1332e8, conferido pela sonda) | Número acima de 2³¹ (ex.: início `4294967296`) dava "não é um número inteiro", mas ele é inteiro, só grande demais. Agora: "início \"4294967296\" é um número fora do limite permitido". | — |
| O1 | Observação | `~` (continuação de sílaba) vira sílaba vazia. É uma escolha válida, porque a sílaba anterior continua soando; o README das fixtures aceita os dois. | — |
| O2 | Observação | Depois de migrar para v2, o 0.1.3 não abre mais o banco (`db_too_new`, como manda a Lei). A volta manual é pelo `masterofnotes.db.bak-v1`. Vale uma linha no guia, se houver downgrade. | Guia (Agente 5), opcional. |

## Parser: fixtures (R1–R4)
**Válidos: 9/9 corretos.**

| Arquivo | Resultado |
|---|---|
| `basic.txt` (critério da SPEC) | (1000, 200, 60, "Lá"), (1200, 200, 62, "ri"), (1900, 200, 64, "la"), (2100, 100, 65, ""); `F`/`R`/`G` fora ✅ |
| `bpm-comma.txt` (`#BPM:150,5`) | (0, 399), (797, 399) ✅ |
| `relative.txt` | (500, 700, 1100, 1400 ms) ✅ |
| `duet.txt` / `duet-p-space.txt` | só P1, `duet = true` ✅ |
| `after-end.txt` | nada depois do `E` ✅ |
| `cp1252.txt` | "Ação", "pé", título "Canção de Teste" ✅ |
| `bom.txt` | título lido apesar do BOM ✅ |
| `crlf.txt` | sem `\r` nas sílabas ✅ |

**Inválidos: 11/11 dão erro em pt-BR, com a linha certa quando a culpa é de uma linha:**
- BPM 0, negativo ou texto → "linha 1: #BPM … não é um número positivo";
- BPM ausente → "linha 3: nota antes do #BPM";
- linha truncada → "linha 4: a nota não tem altura";
- altura 49 → "linha 4: altura 49 (MIDI 109) fora da faixa 24–108";
- altura −37 → linha 3;
- duração −4 → linha 3;
- só F/R/G → "a referência não tem notas";
- arquivo vazio → "falta o #BPM";
- estouro → linha 3 (B1).

**Limite: 4/4 com comportamento sensato:**
- GAP negativo → erro na linha 3 ("a nota cai fora da música");
- sobreposição → normalizada como no JSON e no MIDI (corta a anterior);
- `#bpm: 300` minúsculo e com espaços → aceito;
- nota sem sílaba → sílaba vazia.

**Fuzz leve: 8.493 execuções, 0 pânicos.** Cada fixture foi testada em todos os prefixos (arquivo cortado em cada byte) e em 300 variantes com 3 bytes trocados.

## Migração 002 (R5, Lei 14)
É um portão de release. Replicação independente em Python do executor do `db.rs`: backup com `VACUUM INTO`, `foreign_keys` OFF, uma transação, `foreign_key_check` e `user_version`. O banco 0.1.3 foi montado pelo `001_init.sql`, com dados em **todas** as tabelas:
- 3 músicas (unicode, artista nulo, indisponível);
- 5 referências, com as 4 fontes da 0.1.3, histórico de 3 inativas, offset de +9999 e −40, e status validated/experimental/invalid;
- 2 participantes;
- 2 sessões (festa concluída, duelo ativo);
- 3 rodadas;
- 4 performances: competitiva com rodada, ligadas a referências inativas e sem referência;
- settings com o modelo instalado.

| Checagem | Resultado |
|---|---|
| Todas as 7 tabelas, linha a linha, antes = depois | ✅ |
| Só a definição de `melody_references` mudou (colunas, tipos, defaults e CHECK de status iguais; `'ultrastar'` a mais no CHECK de source) | ✅ |
| `table_info`, FKs de `melody_references` e FK `performances.reference_id` → `melody_references` iguais | ✅ |
| Índice `one_active_reference` recriado igual e ainda valendo (2ª referência ativa da mesma música recusada) | ✅ |
| `foreign_key_check` vazio, `integrity_check` ok, performances ligadas às mesmas referências | ✅ |
| `source = 'ultrastar'` aceito; fonte desconhecida e `song_id` inexistente recusados | ✅ |
| Backup `masterofnotes.db.bak-v1` criado e igual ao banco 0.1.3 (v1, mesmos dados) | ✅ |
| Reabrir: continua v2, nada a fazer | ✅ |
| Referência pendurada (performance → referência inexistente): a migração é desfeita, o banco continua v1 com os dados intactos, sem `melody_references_new` sobrando | ✅ |

Revisão do código (`db.rs`):
- o backup sai antes de qualquer mudança e só quando há dados (v ≥ 1);
- um backup velho da mesma versão é substituído;
- `foreign_keys` volta a ON mesmo se a migração falhar.

O Agente 1 tem os testes Rust equivalentes (`db::tests::migration_002_keeps_013_data_and_accepts_ultrastar`, `migration_with_dangling_reference_rolls_back`). Não os executei, para não compilar o crate Tauri com o disco em cerca de 5,7 GB; a replicação acima é a parte independente.

## E2E (fake + UI)
- `e2e/ultrastar.e2e.ts`:
  - contrato pelo cliente real: `.txt` vira referência `ultrastar` ativa; `notice` só no dueto (omitido nos outros); linha inválida dá `invalid_reference`, com "arquivo UltraStar, linha N";
  - UI: filtro `.txt` no diálogo, sem escolha de trilha, selo "UltraStar", sugestão de alinhar que some ao "Alinhar automaticamente", aviso de dueto, erro com a linha sem trocar a referência ativa.
- `a11y.e2e.ts`: tela "Referência UltraStar recém-importada" em axe, teclado (20/20), viewports e movimento reduzido.
- Suíte completa: 150 passed.
