# Roadmap

Atualizado pelo coordenador. Última atualização: 2026-11-19 (fim da onda 2; MVP com código completo).

## Fase atual: 6–7 (distribuição e estabilização)

| Fase | Estado |
|---|---|
| 0–5 | ✅ Código completo: catálogo, player, captura, referência, pontuação **v3**, Solo/Festa/Duelo/Desafio, rankings, histórico, as 13 telas |
| 6 Distribuição | ✅ **v0.1.1 publicada (pré-release)**: https://github.com/Chamfrado/MasterOfNotes/releases/tag/v0.1.1. Instalador assinado (updater), pontuação v5, guia do usuário |
| 7 Estabilização | 🔄 QA da pontuação v3 (3ª rodada); guia do usuário ✅; ⏳ roteiro manual com microfone real |

Testes em `main`: `pnpm check` (Rust ≈ 160, Vitest 198) + Playwright 99, todos verdes; CI do GitHub verde.
Desempenho: todas as metas cumpridas (`docs/testing/performance.md`, `performance-desktop.md`).

## Próximos passos
1. **Proprietário:** roteiro manual (`docs/testing/evidence/vertical-flow.md`), com o microfone liberado na privacidade do Windows.
2. QA v3 (Agente 4) → ajustes, se houver → release **0.1.1** com o v3, publicado como release **normal** (auto-update).
3. Calibração das tolerâncias com voz real (depende do item 1).

## Bloqueios (ação do proprietário)
| Item | Impacto | Ação necessária |
|---|---|---|
| Licença do produto | `LICENSE` provisório "todos os direitos reservados" | Decidir a licença definitiva |
| Certificado Authenticode (opcional) | aviso do SmartScreen na instalação | Decidir se compra um certificado de assinatura de código |
| Microfone: Realtek padrão mediu −104,8 dBFS | AC-005/006 | Conferir mudo/privacidade do Windows; fazer o teste com voz |
| ffmpeg ausente | Fixtures do checklist de codecs (AC-004) | Instalar ffmpeg ou fornecer vídeos de teste próprios |

## Riscos
- Codecs MKV/AVI no WebView2 (ADR-0003): mitigado com mensagem clara; FFmpeg adiado.
- **Voz-guia do vídeo captada pelo microfone (A2): a −20 dB pontua 99,8 com o usuário calado.** Risco aceito até T-004-06 (checagem de vazamento obrigatória para o modo competitivo).
- YIN: robusto com voz sintética realista (revisão QA). Regras do scorer v1 com injustiças A1–A9, corrigidas no v2 (T-006-06).
- Disco C: com pouca folga: política de builds por crate (docs/agent-coordination.md).

## Decisões pendentes
Nenhuma bloqueando a onda 2.
