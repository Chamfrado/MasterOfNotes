# Estratégia de testes

| Camada | Ferramenta | Onde | Roda no CI |
|---|---|---|---|
| DSP (pitch, VAD) | `cargo test` + sinais sintéticos gerados em código | `crates/audio-engine/tests` | ✅ |
| Pontuação | `cargo test` + regressão com tabela de esperados | `crates/scoring-engine/tests` | ✅ |
| Regras de jogo | `cargo test` | `crates/game` | ✅ |
| Persistência / scanner | `cargo test` com tempdir e SQLite em arquivo temporário | `apps/desktop/src-tauri` | ✅ |
| Componentes / máquina do player | Vitest + Testing Library (jsdom) | `apps/desktop/src/**/*.test.ts(x)` | ✅ |
| Fluxos de UI | Playwright contra `vite` com IPC mockado **só no teste** | `apps/desktop/e2e` | ✅ |
| Integração desktop real | Checklist manual com evidência | `docs/testing/evidence/` | ❌ (exige hardware) |

## Separação automatizável × hardware real
Microfone físico, codecs no WebView2 real, instalador e atualização são verificados manualmente com checklist datado em `docs/testing/evidence/`. Nunca declarar como testado sem registro.

## Fixtures
- Áudio: **gerado em código** (senoides, harmônicos, ruído com seed fixa) — reproduzível, sem binários no repo, sem problema de licença.
- Referências: JSON/MIDI pequenos de autoria própria em `tests/fixtures/references/`.

## Performance (metas a medir; números só após medição no ambiente de referência)
Ambiente de referência: a máquina de desenvolvimento (registrar CPU/RAM no relatório).
| Métrica | Meta |
|---|---|
| Inicialização até catálogo visível | < 2 s |
| Indexação de 1.000 vídeos (primeira vez) | < 30 s |
| Latência de feedback (frame → UI) | < 80 ms |
| CPU durante performance | < 15% de um núcleo para análise |
