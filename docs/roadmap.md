# Roadmap

Atualizado pelo coordenador. Última atualização: 2026-11-19.

## Fase atual: 0 → 1 (Discovery concluído, Fundação em paralelo)

| Fase | Saída | Estado |
|---|---|---|
| 0 Discovery | Repo, leis, specs, contratos, equipe | ✅ concluída |
| 1 Fundação | App roda, persiste configurações, CI | 🔄 |
| 2 Catálogo e reprodução | Pasta → lista → vídeo toca | 🔄 (paralelo à 1) |
| 3 Motor musical | Casos conhecidos avaliados corretamente | 🔄 (crate puro, paralelo) |
| 4 Karaokê completo | Primeira partida completa salva e no ranking | ⏳ |
| 5 Modos sociais | Festa completa | ⏳ |
| 6 Distribuição | Instalador + update assinado testado | ⏳ |
| 7 Estabilização | Release candidate | ⏳ |

## Em andamento (onda 1)

| Agente | Tarefas |
|---|---|
| 1 | T-001-02..05, T-002-01..03 (db, settings, scanner, watcher, commands) |
| 2 | T-004-01, T-005-01, T-006-01, T-006-02 (YIN, referência, scorer, regressão) |
| 3 | T-010-01, T-010-02, T-003-01, T-002-04 (tokens, componentes, máquina do player, Biblioteca) |
| 4 | T-008-01, T-007-01, CI (`.github/workflows/ci.yml`), infraestrutura de testes |

## Próximas (onda 2, após integração da onda 1)
T-004-02/03 (captura real) → T-004-04 (glue) → T-006-03 (performance.rs) → T-003-02/03 (Player) → T-005-03/04 → T-006-05 → T-007-02/03 → **fluxo vertical completo**.

## Bloqueios
| Item | Impacto | Ação necessária |
|---|---|---|
| Licença do produto | `LICENSE` provisório "todos os direitos reservados" | Proprietário decidir licença definitiva |
| Chave de assinatura do updater + secrets GitHub | SPEC-009 (AC-015) | Proprietário autorizar geração/armazenamento |
| Teste com microfone físico e vídeos reais | AC-004/005/006 | Proprietário executar checklist manual (ou confirmar dispositivo disponível) |

## Riscos
- Codecs MKV/AVI no WebView2 (ADR-0003) — mitigado com mensagem clara; FFmpeg adiado.
- Vazamento da música no microfone em caixas de som — mitigado por aviso e recomendação de fones.
- Precisão do YIN em vozes graves/soprosas — medida pela regressão; plano B: pYIN.

## Decisões pendentes
Nenhuma bloqueando a onda 1.
