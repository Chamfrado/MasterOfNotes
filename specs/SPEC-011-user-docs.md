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
| T-011-01 | Estrutura, índice e plano de mídia (lista de capturas por página) | Agente 5 | todo |
| T-011-02 | Script de captura (screenshots + GIF + vídeo) reprodutível | Agente 5 | todo |
| T-011-03 | Páginas do guia (R2) com mídia | Agente 5 | todo |
| T-011-04 | Revisão de exatidão contra a UI + revisão do coordenador | Agente 5 / Agente 0 | todo |

## STATUS
Criada em 2026-11-19 a pedido do proprietário.
