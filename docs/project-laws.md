# Leis do Projeto

Imutáveis, salvo ADR aprovada pelo coordenador (Agente 0). Todo agente, humano ou IA, está sujeito a elas.

| # | Lei | Na prática |
|---|-----|-----------|
| 01 | **Spec First** | Funcionalidade relevante tem critérios de aceite em `specs/` antes do código. |
| 02 | **Test Before Done** | Comportamento crítico só está pronto com teste executado e passando. |
| 03 | **No Fake Functionality** | Proibido simular microfone, pontuação, atualização, importação ou persistência e apresentar como real. Mocks só em testes/dev, nunca no caminho de produção. |
| 04 | **Single Source of Truth** | Cada dado tem um dono: SQLite (via Rust) para dados persistentes; `ipc.ts` para o contrato frontend↔backend; `specs/` para requisitos. |
| 05 | **Separation of Concerns** | Domínio musical (`crates/scoring-engine`), dispositivos (`crates/audio-engine`), regras de jogo (`crates/game`), persistência/infra (`src-tauri`), UI (`apps/desktop/src`). |
| 06 | **No Unnecessary Complexity** | Sem abstração especulativa, sem dependência dispensável. Ponytail é a régua. |
| 07 | **Explicit Contracts** | Interfaces compartilhadas (`docs/architecture/contracts.md`, `ipc.ts`) mudam só via coordenador. |
| 08 | **Performance Matters** | Nada pesado na thread de UI nem no callback de áudio. Varredura, hashing e DSP fora da thread principal. |
| 09 | **Secure by Default** | Menor privilégio nas capabilities Tauri; escopo de asset protocol só para a pasta escolhida; validar toda entrada do IPC. |
| 10 | **Offline First** | Tudo funciona sem internet. A rede só é usada para checar atualização, com consentimento. |
| 11 | **Versioned Data** | Migrações SQLite numeradas; formato de referência versionado; `ALGORITHM_VERSION` gravado em cada performance. |
| 12 | **Reproducibility** | Lockfiles versionados (`pnpm-lock.yaml`, `Cargo.lock`); scoring determinístico; fixtures versionadas. |
| 13 | **Observable Failures** | Erros chegam ao usuário com mensagem compreensível e ao log com contexto. Sem coletar dado sensível. |
| 14 | **Protect User Data** | Migração/atualização nunca apaga dado do usuário sem consentimento. Áudio do microfone não é gravado. |
| 15 | **No Silent Scope Expansion** | Serviço externo ou sistema complexo fora do escopo exige ADR. |
| 16 | **Accessibility and UX** | Todo fluxo operável por teclado, contraste AA, `prefers-reduced-motion` respeitado. |
| 17 | **Real Evidence** | Relatórios separam: executado ✅, pendente ⏳, suposição ❓. Nunca declarar teste que não rodou. |
| 18 | **Controlled Integration** | Ninguém sobrescreve arquivo de outro dono. Integração em `main` só pelo coordenador, com testes verdes. |
| 19 | **Copyright and Licensing** | Não redistribuir vídeos, músicas, letras, modelos ou codecs de terceiros. Fixtures de teste são sintéticas ou com licença verificada. |
| 20 | **Product Integrity** | O fluxo vertical completo vale mais que qualquer funcionalidade decorativa. |

## Validação automatizada

- CI (`.github/workflows/ci.yml`) roda typecheck, testes TS, `cargo test --workspace`, `cargo clippy -D warnings` e `cargo fmt --check`.
- `cargo test -p scoring-engine` inclui a suíte de regressão do algoritmo (Lei 11/12).
- Busca proibida no CI: `Math.random` fora de `*.test.*` dentro de `apps/desktop/src/**/scor*` (Lei 03).
