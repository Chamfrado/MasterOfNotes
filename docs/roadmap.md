# Roadmap

Atualizado pelo coordenador. Última atualização: 2026-11-19 (fim da onda 1).

## Fase atual: 1–3 concluindo → 4 (karaokê completo)

| Fase | Saída | Estado |
|---|---|---|
| 0 Discovery | Repo, leis, specs, contratos, equipe | ✅ |
| 1 Fundação | App roda, persiste configurações, CI | ✅ código + testes; CI nunca rodou no GitHub (sem push) |
| 2 Catálogo e reprodução | Pasta → lista → vídeo toca | 🔄 backend + Biblioteca prontos; player completo e validação no app real pendentes |
| 3 Motor musical | Casos conhecidos avaliados corretamente | ✅ 12/12 critérios da SPEC-006 (sintético); ⏳ voz real |
| 4 Karaokê completo | Primeira partida completa salva e no ranking | 🔄 onda 2 |
| 5 Modos sociais | Festa completa | 🔄 regras prontas (`crates/game`); commands e telas na onda 2/3 |
| 6 Distribuição | Instalador + update assinado testado | 🔄 workflow de release pronto; bloqueado pela chave |
| 7 Estabilização | Release candidate | ⏳ |

## Onda 1: concluída (23 tarefas done)
db/migrações/backup, settings, scanner + watcher + commands da biblioteca, YIN/VAD, captura cpal, calibração, referência JSON/MIDI, scorer v1 + regressão, métricas exploratórias, regras de jogo, CI, workflow de release (draft), design system, máquina do player, Biblioteca e telas do fluxo ligadas ao IPC real.
Testes em `main`: Rust 13 suítes verdes (≈ 88 testes + 3 de hardware ignorados no CI), Vitest 105/105.

## Em andamento (onda 2)

| Agente | Tarefas |
|---|---|
| 1 | T-004-04 (commands de mic), T-005-03 (commands de referência), glue do performance + T-006-05, T-008-02/T-007-02, escopo de asset por extensão, T-009-01 (NSIS) |
| 2 | T-006-03 `performance.rs` (§2.4) + `Scorer::skip` |
| 3 | T-003-02/03 (player completo), T-004-05 (tela de microfone), T-005-04 (tela de referência) |
| 4 | Revisão QA independente do scoring → E2E Playwright (smoke, depois Festa T-008-04) |

## Próximo marco
**Fluxo vertical completo no app real**: pasta → vídeo → microfone → calibração → canto → pontuação contra referência → salvar → ranking. Validação manual com evidência em `docs/testing/evidence/`.

## Bloqueios (ação do proprietário)
| Item | Impacto | Ação necessária |
|---|---|---|
| Licença do produto | `LICENSE` provisório "todos os direitos reservados" | Decidir a licença definitiva |
| Chave de assinatura do updater + secrets GitHub | AC-015, release | Gerar a chave (`pnpm --filter desktop tauri signer generate`, fora do repo) e criar os secrets `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)` |
| Push para `origin` | CI/release nunca rodaram no GitHub | Autorizar o push |
| Microfone: Realtek padrão mediu −104,8 dBFS | AC-005/006 | Conferir mudo/privacidade do Windows; fazer o teste com voz |
| ffmpeg ausente | Fixtures do checklist de codecs (AC-004) | Instalar ffmpeg ou fornecer vídeos de teste próprios |

## Riscos
- Codecs MKV/AVI no WebView2 (ADR-0003): mitigado com mensagem clara; FFmpeg adiado.
- **Voz-guia do vídeo captada pelo microfone (A2): a −20 dB pontua 99,8 com o usuário calado.** Risco aceito até T-004-06 (checagem de vazamento obrigatória para o modo competitivo).
- YIN: robusto com voz sintética realista (revisão QA). Regras do scorer v1 com injustiças A1–A9, corrigidas no v2 (T-006-06).
- Disco C: com pouca folga: política de builds por crate (docs/agent-coordination.md).

## Decisões pendentes
Nenhuma bloqueando a onda 2.
