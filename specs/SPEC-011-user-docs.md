# SPEC-011 — Documentação do usuário

## SPEC
Guia minucioso para quem nunca usou o app, em pt-BR, com **imagens, GIFs e vídeos curtos**, legível direto no GitHub (repo público) e offline.

- R1. Local: `docs/user/` (um arquivo por tema) + índice `docs/user/README.md`, linkado no `README.md` da raiz.
- R2. Cobertura mínima: instalação e desinstalação (Windows); primeira execução; escolher a pasta e formatos suportados (o que fazer com MKV/AVI); Biblioteca, busca e Detalhes da música; microfone, calibração, teste de vazamento e latência (manual e automática); referência melódica (o que é, por que é necessária, como importar MIDI/JSON, escolher a trilha, alinhar e validar); cantar (player, atalhos, overlay, treino x competitivo); como a pontuação funciona (afinação/ritmo/sustentação, dificuldades, oitava, por que volume não conta); modos Solo, Festa, Duelo e Desafio; ranking e histórico ("versão anterior"); Ajustes, Modo TV, teclado/controle remoto (setas); atualizações; privacidade (áudio do microfone nunca é gravado); solução de problemas e FAQ; glossário.
- R3. Mídia: screenshots PNG, GIFs curtos (≤ 15 s) para fluxos e vídeos (WebM/MP4) curtos linkados. **O GitHub não renderiza `<video>` de arquivos do repositório**: no texto, o GIF é o formato principal, e o vídeo é um link opcional.
- R4. **Orçamento**: GIF ≤ 2 MB cada; vídeo ≤ 5 MB cada; mídia total ≤ 40 MB (repo público, histórico permanente). Resolução 1280×800, tema padrão, mais 1 exemplo em Modo TV.
- R5. Mídia gerada por **script reprodutível** (Playwright + o mock/fake-backend do E2E) em `apps/desktop/e2e/docs/`. Rodar o script de novo regenera tudo após mudanças de UI. Dados de exemplo são **fictícios e marcados como exemplo** (Lei 19: sem conteúdo de terceiros; usar a música de teste e títulos inventados). Números de pontuação nas imagens são ilustrativos, e a legenda diz isso (Lei 03).
- R6. Exatidão: cada passo descrito foi conferido contra o código atual (rótulos de botões iguais aos da UI). Funcionalidades não validadas com hardware real são descritas sem prometer resultado.
- R7. Acessível: todo GIF/imagem tem texto alternativo descritivo; os passos também estão em texto (nunca só na imagem).

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-011-01 | Estrutura, índice e plano de mídia (lista de capturas por página) | Agente 5 | done |
| T-011-02 | Script de captura (screenshots + GIF + vídeo) reprodutível | Agente 5 | done |
| T-011-03 | Páginas do guia (R2) com mídia | Agente 5 | done |
| T-011-04 | Revisão de exatidão contra a UI + revisão do coordenador | Agente 5 / Agente 0 | done |

## STATUS
Criada em 2026-11-19 a pedido do proprietário.
- T-011-01 `review` (Agente 5, 2026-11-19): índice e sumário detalhado em `docs/user/README.md` (14 páginas, rótulos conferidos contra `apps/desktop/src/screens/*`); plano de capturas por página em `apps/desktop/e2e/docs/PLANO-DE-MIDIA.md` (~33 PNG, ~9 GIF, 2 vídeos, ~20 MB estimados). Proposta de GIF: screenshots + `gifenc` (devDependency, via Agente 3). Páginas ainda não escritas (T-011-03).
- T-011-02 `review` (Agente 5, 2026-11-19): `apps/desktop/e2e/docs/` (config própria na porta 1450, `capture.docs.ts`, `media.ts`). Importa tauri-mock/fixtures/fake-backend sem alterar; vídeo = música de teste; feedback ao vivo e medidor injetados nos Channels (ilustrativos). GIF via gifenc (960×600, 128 cores, pixels inalterados transparentes). Regeneração completa ✅ 13/13 em 4,4 min; 47 arquivos, 9,8–10,9 MB, todos dentro do R4. Mídia ainda não commitada (só depois da regeneração final, para não gravar versões intermediárias no histórico).
- T-011-03 `review` (Agente 5, 2026-11-19): 14 páginas em `docs/user/` cobrindo o R2; rótulos e mensagens conferidos no código (telas, `live.rs`, `performance.rs`, `scorer.rs`, `reference.rs`, `plan_rounds`). Links, âncoras, mídias e textos alternativos verificados por script ✅. Já escrito com as mudanças anunciadas (solfejo, Resultado em %, reprovação do vazamento só com a mensagem do Rust). ⏳ Regenerar imagens após o merge de agent-3 (c58174c, solfejo) e agent-4 (c8235d5, targetScore). ⏳ Screenshots do instalador após o release v0.1.0 (placeholders comentados em 01-instalacao.md).
- 2026-11-19 (Agente 5): guia alinhado ao main 7caa1c9 (solfejo, Resultado em % com o motivo do treino, duelo com margem, algoritmo v3: vibrato, notas curtas, áudio perdido). Mídia regenerada uma única vez e commitada: 48 arquivos (38 PNG, 9 GIF, 1 WebM), 10,0 MB. Regeneração ✅ 13/13 (4,0 min); verificador de links ✅. ⏳ Screenshots do instalador quando o v0.1.0 for publicado.
- T-011-01..04 done (Agente 0, 2026-11-19): guia integrado em main @ deccdeb. 14 páginas, 48 arquivos de mídia (10,0 MB, dentro do R4), gerados por script reprodutível. Revisão de exatidão do coordenador: fórmula, pesos e tolerâncias v3, regras (oitava, volume, vibrato, áudio perdido), rótulos de feedback e 10 botões citados conferidos contra o código; capturas de Resultado (treino) e Player conferidas. ⏳ Screenshots do instalador quando o v0.1.0 for publicado.
