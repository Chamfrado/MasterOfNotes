# Auditoria de dependências e licenças (T-014-02, SPEC-014 R2)

Agente 1, 2026-11-20. Código: `main` @ 9877d74 (versão 0.1.4). Nenhuma dependência foi atualizada nesta auditoria.

## Resumo

| Item | Resultado |
|---|---|
| Vulnerabilidades JS, produção | ✅ nenhuma conhecida |
| Vulnerabilidades JS, desenvolvimento | ✅ nenhuma conhecida |
| Vulnerabilidades Rust | ✅ nenhuma. 6 avisos (5 "unmaintained", 1 "unsound"), **nenhum** compilado no build do Windows |
| Licenças das dependências diretas | ✅ todas permissivas (MIT, Apache-2.0, Unlicense, CC0, OFL-1.1). Nenhuma GPL, AGPL ou LGPL e nenhuma desconhecida |
| Licenças do que vai no instalador | ⚠️ 1 crate MPL-2.0 (`option-ext`) e 2 fontes OFL-1.1: exigem aviso |
| `THIRD-PARTY-NOTICES.txt` cobre a distribuição? | ❌ **não**: cobre só a ONNX Runtime (ver Ações) |

## 1. Vulnerabilidades

### JS (pnpm 10.30.3, raiz do monorepo)
- `pnpm audit --prod`: `No known vulnerabilities found`.
- `pnpm audit --dev`: `No known vulnerabilities found`.

### Rust (`cargo audit` 0.23.0)
- **Ferramenta:** `cargo install cargo-audit --locked`, compilado do código-fonte do crates.io. O target ficou temporário no scratchpad e foi apagado depois. Ocupa 17 MB em `~/.cargo/bin` e 54 MB do banco RustSec em `~/.cargo/advisory-db`.
- **Varredura:** 1.550 avisos do banco RustSec contra o `Cargo.lock` (602 crates de todas as plataformas). **0 vulnerabilidades.** Os 6 avisos:

| Crate | Aviso | Quem traz | No build Windows? |
|---|---|---|---|
| chrono 0.4.45 | RUSTSEC-2026-0543, unmaintained | dependência opcional de `serde_with`, não ativada | não (nem é compilado) |
| proc-macro-error 1.0.4 | RUSTSEC-2024-0370, unmaintained | `glib-macros` (GTK) | não (só Linux) |
| zbus_names 4.3.5 | RUSTSEC-2026-0371, unmaintained | `zbus` ← `tauri-plugin-opener` | não (D-Bus, só Linux) |
| zvariant 5.15.1 | RUSTSEC-2026-0370, unmaintained | `zbus` | não (só Linux) |
| zvariant_derive 5.15.1 | RUSTSEC-2026-0372, unmaintained | `zbus` | não (só Linux) |
| glib 0.18.5 | RUSTSEC-2024-0429, unsound (`VariantStrIter`) | `gtk` ← `tauri`/`tao`/`wry` | não (só Linux) |

Conferido com `cargo tree -p master-of-notes --target x86_64-pc-windows-msvc -e normal,build -i <crate>`: "nothing to print" nos seis. Eles só chegam no build Linux do Tauri (GTK/D-Bus), que o produto não distribui. Não há ação agora; as correções virão por atualizações do próprio Tauri.

## 2. Licenças

### Dependências diretas, Rust (Cargo.toml de cada crate; resolvidas pelo `Cargo.lock`)

| Crate do projeto | Dependência | Versão | Tipo | Licença |
|---|---|---|---|---|
| master-of-notes | blake3 | 1.8.7 | normal | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception |
| | log | 0.4.35 | normal | MIT OR Apache-2.0 |
| | notify-debouncer-full | 0.7.0 | normal | MIT OR Apache-2.0 |
| | rusqlite (bundled: SQLite em domínio público) | 0.43.0 | normal | MIT |
| | serde / serde_json | 1.0.229 / 1.0.154 | normal | MIT OR Apache-2.0 |
| | tauri | 2.12.7 | normal + dev | Apache-2.0 OR MIT |
| | tauri-build | 2.7.6 | build | Apache-2.0 OR MIT |
| | tauri-plugin-dialog / -log / -opener / -updater | 2.8.4 / 2.10.2 / 2.7.3 / 2.14.1 | normal | Apache-2.0 OR MIT |
| | tempfile | 3.27.0 | dev | MIT OR Apache-2.0 |
| audio-engine | cpal | 0.15.3 | normal | Apache-2.0 |
| | rtrb | 0.3.6 | normal | MIT OR Apache-2.0 |
| | serde, thiserror 2.0.21 | | normal | MIT OR Apache-2.0 |
| game | serde, thiserror; serde_json (dev) | | | MIT OR Apache-2.0 |
| scoring-engine | midly | 0.5.3 | normal | Unlicense |
| | serde, serde_json, thiserror | | normal | MIT OR Apache-2.0 |
| song-analysis | ort (load-dynamic) | 2.0.0-rc.15 | normal | MIT OR Apache-2.0 |
| | rustfft | 6.4.1 | normal | MIT OR Apache-2.0 |
| | sha2 | 0.10.9 | normal | MIT OR Apache-2.0 |
| | thiserror | 2.0.21 | normal | MIT OR Apache-2.0 |

### Dependências diretas, JS (`apps/desktop/package.json`; licença via `pnpm licenses list`)

| Pacote | Tipo | Licença |
|---|---|---|
| react, react-dom (+ scheduler) | produção | MIT |
| @tauri-apps/api | produção | Apache-2.0 OR MIT |
| @tauri-apps/plugin-dialog / -opener / -updater | produção | MIT OR Apache-2.0 |
| @fontsource-variable/inter, @fontsource-variable/bricolage-grotesque | produção (as fontes vão no bundle) | **OFL-1.1** |
| Desenvolvimento (17 diretas, 109 pacotes no total) | dev, **não distribuídas** | MIT (89), Apache-2.0 (8), BSD (4), ISC (2), BlueOak-1.0.0 (2), Apache-2.0 OR MIT (1), **MPL-2.0 (3: axe-core, @axe-core/playwright, lightningcss)** |

As três MPL-2.0 de desenvolvimento são ferramentas de teste e de build: não entram no instalador e não impõem nada ao produto.

### O que vai no instalador (Windows)
- **Rust:** 304 crates compilados no `master-of-notes.exe` (`cargo tree --target x86_64-pc-windows-msvc -e normal`).
  - Licenças, na contagem conservadora de todas as plataformas (450 crates): MIT/Apache-2.0 em várias combinações (≈ 400), Unicode-3.0 (18), ISC (5), BSD-3-Clause (3+), Zlib (2), Unlicense/CC0/0BSD.
  - Nenhuma GPL, AGPL ou LGPL, e nenhum crate sem licença SPDX.
- **MPL-2.0 no binário:** só `option-ext` 0.2.0 (← `dirs-sys` ← `dirs` ← `tauri`). Os outros quatro MPL-2.0 do grafo (`cssparser`, `cssparser-macros`, `selectors`, `dtoa-short`) aparecem só dentro de `tauri-codegen`/`tauri-macros` (proc-macro, tempo de compilação) e não vão no executável.
  - A MPL-2.0 é copyleft por arquivo: o uso em produto proprietário é permitido, desde que não modifiquemos os arquivos MPL (não modificamos) e que o destinatário seja informado de onde obter o código-fonte desses arquivos (§3.2).
- **JS (bundle da WebView):** React, scheduler e @tauri-apps/* (MIT/Apache), mais as fontes Inter e Bricolage Grotesque (OFL-1.1). A OFL permite embutir as fontes em software proprietário, desde que a licença e o aviso de copyright acompanhem as fontes, que não podem ser vendidas isoladamente.
- **Binários de terceiros:** `onnxruntime.dll` 1.31.1 (MIT, Microsoft), com o SHA-256 fixado no script de build e já citado no `THIRD-PARTY-NOTICES.txt`. O SQLite vem embutido via `rusqlite`/bundled e está em domínio público, sem exigência.
- **Modelos de separação:** não são distribuídos. O usuário baixa da fonte e aceita os termos (SPEC-012 R1). Fora do escopo do aviso.

## 3. O `THIRD-PARTY-NOTICES.txt` cobre a distribuição?
**Não.** Hoje ele cobre só a ONNX Runtime (MIT, texto completo) e explica que os modelos não são distribuídos. Faltam:
1. **Avisos das licenças MIT, BSD, ISC e Unicode** dos crates compilados no executável e do JS do bundle (React etc.). Essas licenças exigem que o aviso de copyright e o texto da licença acompanhem "cópias ou partes substanciais", e o binário e o bundle minificado são cópias. O Vite remove os comentários de licença do bundle, então hoje esses avisos não estão em lugar nenhum do instalador.
2. **Apache-2.0:** reproduzir o arquivo NOTICE dos crates que tenham um. No registro local, só `cfg_aliases` 0.2.2 tem um (`NOTICES.md`), e ele é dependência de build script, fora do binário. Hoje não há NOTICE a reproduzir, mas o gerador do item 1 de Ações cobre isso no futuro.
3. **MPL-2.0 (`option-ext`):** dizer que o crate é MPL-2.0 e onde está o código-fonte (crates.io/GitHub).
4. **OFL-1.1:** texto da OFL e copyright das duas fontes.

## 4. Ações recomendadas (nenhuma aplicada; dependem da decisão do coordenador)
1. **[Alta, distribuição] Gerar os avisos completos.** A proposta é o `cargo-about` (ferramenta de build, MIT/Apache, mantida pela Embark). Ele produz o texto das licenças de todos os crates do grafo Windows a partir do `Cargo.lock`, e um trecho do mesmo script acrescenta os pacotes JS de produção (`pnpm licenses list --prod --json` dá o caminho de cada pacote, e o texto sai do arquivo LICENSE).
   - O resultado vira o `THIRD-PARTY-NOTICES.txt` empacotado: a ONNX Runtime continua no topo, depois as seções Rust, JS e fontes, com a nota de fonte da MPL.
   - Duas formas: (a) gerar no release.yml antes do tauri-action, sem nada commitado; ou (b) commitar o arquivo gerado e ter um passo de CI que falha se o `Cargo.lock`/`pnpm-lock.yaml` mudar sem regenerar.
   - Recomendo (a), que é o mesmo padrão do fetch da DLL. É uma ferramenta nova no build, por isso fica para a sua aprovação.
2. **[Média] Política de licenças no CI:** um `deny.toml` com a lista de licenças permitidas (MIT, Apache-2.0, BSD-2/3, ISC, Zlib, Unicode-3.0, Unlicense, CC0-1.0, 0BSD, MPL-2.0 só por exceção nominal) mais `cargo deny check licenses advisories`. Assim uma dependência GPL ou um aviso novo quebra o build antes do release.
3. **[Baixa] `cargo audit` no CI** (ou o `advisories` do item 2), para avisos novos aparecerem sem auditoria manual.
4. **[Nenhuma agora] Avisos unmaintained/unsound:** todos fora do build Windows. Reavaliar quando houver versão Linux ou quando o Tauri atualizar `gtk`/`zbus`.

## Reproduzir
```sh
pnpm audit --prod && pnpm audit --dev
cargo install cargo-audit --locked && cargo audit
cargo tree -p master-of-notes --target x86_64-pc-windows-msvc -e normal -i <crate>
pnpm licenses list --prod -r
cargo metadata --format-version 1 --locked   # licença de cada pacote (campo "license")
```
