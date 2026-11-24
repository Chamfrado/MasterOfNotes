# Roadmap

Atualizado pelo coordenador. Última atualização: 2026-11-24 (v0.1.7 publicada; Fase 7 concluída no que não depende do proprietário).

## Fase atual: 7 (estabilização)

| Fase | Estado |
|---|---|
| 0–5 | ✅ Código completo: catálogo, player, captura, referência, pontuação **v5**, Solo/Festa/Duelo/Desafio, rankings, histórico, as 13 telas |
| 6 Distribuição | ✅ **v0.1.7 publicada (release normal, `latest`)**: https://github.com/Chamfrado/MasterOfNotes/releases/latest. Atualização automática ativa desde a 0.1.2; histórico 0.1.1–0.1.6 abaixo |
| 7 Estabilização | 🔄 SPEC-014: segurança ✅, dependências ✅, arquitetura ✅, Ponytail ✅, docs técnicas ✅; QA sintético da pontuação encerrado no v5; guia do usuário ✅; ⏳ roteiro manual com microfone real |

Testes em `main`: `pnpm check` (cargo test + Vitest 224) + Playwright 151 + `cargo deny` (avisos e licenças, também semanal), todos verdes; CI do GitHub verde.
Desempenho: todas as metas cumpridas (`docs/testing/performance.md`, `performance-desktop.md`).

## Backlog 0.1.2 (entregue na v0.1.2, exceto o roteiro manual e a calibração com voz real)
- **Publicar como release NORMAL** (não pré-release): sem nenhuma release normal, `releases/latest/download/latest.json` dá 404, e a verificação de atualização da 0.1.1 mostra erro. A primeira release normal resolve isso para quem está na 0.1.1.
- UI de Atualizações: tratar 404 do endpoint como "nenhuma versão nova pelo canal automático" + link para a página de releases (Agente 3).
- UI da sonda de latência: exibir o jitter e recomendar repetir se > 10 ms (SPEC-006 v3 N4) (Agente 3).
- Achados do roteiro manual do proprietário e calibração com voz real (SPEC-006, backlog M1/M2/B1).

## Janela autônoma (2026-11-20, 24 h, autorizada pelo proprietário)
Releases automáticas conforme [release-process.md](release-process.md).
- **0.1.3 ✅ PUBLICADA (2026-11-20):** referência automática a partir do vídeo (SPEC-012 / ADR-0004 B, modelo baixado pelo usuário) + alinhamento automático de referências.
- **0.1.4 ✅ PUBLICADA (2026-11-23, fora da janela, aprovada pelo proprietário):** importação de UltraStar `.txt` (notas + letra; ADR-0002); avisos de licença completos; workflow de release endurecido.
- **0.1.5 ✅ PUBLICADA (2026-11-23, aprovada pelo proprietário):** correções da revisão de segurança (B1, B3–B6).
- **0.1.6 ✅ PUBLICADA (2026-11-23, aprovada pelo proprietário):** aviso quando o microfone falha no meio da música (`audio://error`, Lei 13) + limpeza da auditoria Ponytail.
- **0.1.7 ✅ PUBLICADA (2026-11-24, aprovada pelo proprietário):** aviso de "voz fraca" salvo com a referência extraída (migração 003, T-012-08); `cargo deny` no CI (T-014-06); `cookie`/`url` atualizados (RUSTSEC-2026-0594/0592); limpezas.
- Backlog (vindo do QA da 0.1.3): B2, viés de ±31–36 ms no alinhamento (só registro).
- Fora da janela (dependem do proprietário): calibração com voz real, mudanças na pontuação, licença do produto, análise jurídica do modelo opcional.

## Próximos passos
1. **Proprietário:** roteiro manual (`docs/testing/evidence/vertical-flow.md`), com o microfone liberado na privacidade do Windows.
2. Calibração das tolerâncias com voz real (depende do item 1).

## Bloqueios (ação do proprietário)
| Item | Impacto | Ação necessária |
|---|---|---|
| Licença do produto | `LICENSE` provisório "todos os direitos reservados" | Decidir a licença definitiva |
| Certificado Authenticode (opcional) | aviso do SmartScreen na instalação | Decidir se compra um certificado de assinatura de código |
| Microfone: Realtek padrão mediu −104,8 dBFS | AC-005/006 | Conferir mudo/privacidade do Windows; fazer o teste com voz |
| ffmpeg ausente | Fixtures do checklist de codecs (AC-004) | Instalar ffmpeg ou fornecer vídeos de teste próprios |

## Riscos
- Codecs MKV/AVI no WebView2 (ADR-0003): mitigado com mensagem clara; FFmpeg adiado.
- **Voz-guia do vídeo captada pelo microfone (A2): a −20 dB pontua 99,8 com o usuário calado.** Mitigado por T-004-06 (done): o modo competitivo exige checagem de vazamento aprovada para o dispositivo e o volume atuais.
- YIN: robusto com voz sintética realista (revisão QA). Regras do scorer v1 com injustiças A1–A9, corrigidas no v2 (T-006-06).
- Disco C: com pouca folga: política de builds por crate (docs/agent-coordination.md).

## Decisões pendentes
As do proprietário estão em "Bloqueios" e na janela autônoma (licença, análise jurídica do modelo opcional, rotação opcional da chave de assinatura).
