# SPEC-010 — Design system e telas

## SPEC
- Tema escuro padrão (grafite), uma cor de destaque vibrante, gradientes com moderação. Tokens (cor, espaço, raio, sombra, duração, easing, tipografia) no `@theme` do Tailwind v4 — sem valores soltos em componentes.
- Escala "10-foot": legível em TV; alvos ≥ 48 px; foco visível de alto contraste.
- Componentes: Button, Input, Card, List, RankingTable, Dialog, Toast, Progress, MicMeter, PitchIndicator, ScoreBadge, Select, Slider, NavBar.
- Telas: Início, Biblioteca, Detalhes da música, Microfone, Referência, Seleção de modo, Participantes, Player, Resultado, Ranking, Histórico de festas, Configurações, Atualizações.
- Estados reais de loading/vazio/erro em toda tela que busca dados. Nenhum dado fictício fora de testes (Lei 03).
- Teclado: toda tela navegável por Tab/Enter/Esc; atalhos do player. `prefers-reduced-motion` e o setting `reducedMotion` desativam animações não essenciais.
- Contraste WCAG AA. Fontes empacotadas localmente (offline).
- Animações só com `transform`/`opacity`; meta de 60 FPS medida em hardware de referência antes de ser declarada.

## TASKS
| ID | Tarefa | Dono | Status |
|---|---|---|---|
| T-010-01 | Tailwind v4 + tokens + fontes locais + layout/navegação | Agente 3 | done |
| T-010-02 | Componentes base + testes de acessibilidade (role/label/foco) | Agente 3 | done |
| T-010-03 | Telas do fluxo vertical (Início, Biblioteca, Microfone, Player, Resultado, Ranking) | Agente 3 | done |
| T-010-04 | Telas sociais, Configurações, Atualizações | Agente 3 | done |
| T-010-05 | Auditoria a11y/10-foot via Playwright (axe, teclado, viewports, movimento reduzido) | Agente 4 | done |
| T-010-06 | Tela Detalhes da música (editar, arquivo, reprodução, referência, recordes, Cantar/Ranking) | Agente 3 | done |

## STATUS
2026-11-19 (Agente 3):
- T-010-01 em review. Tokens no `@theme` (`src/styles.css`): grafite + destaque `#ff4f8b`, gradiente só em marca/placar, Inter + Bricolage Grotesque empacotadas (`@fontsource-variable`, saem no bundle, sem rede). Escala 10-foot: `font-size` raiz `clamp(16px, 0.55vw + 10px, 28px)`; alvos `min-h-12`; foco `3px` amarelo `--color-focus`. `prefers-reduced-motion` e `data-reduced-motion` (setting `reducedMotion`) zeram animações. Navegação: pilha de rotas em memória (`src/nav.ts`), Esc = voltar, foco vai para o `<main>` a cada tela.
- T-010-02 em review. `components/ui.tsx` (Button, Input, Select, Slider, Card, Badge, Progress, Dialog, Toast, NavBar, Loading/ErrorState) e `components/game.tsx` (MicMeter, PitchIndicator, ScoreBadge, RankingTable). Nativos: `<dialog>`, `<select>`, `<progress>`, `<input type=range>`, `<table>`. Sem Radix/motion. Testes: `ui.test.tsx`, `game.test.tsx`.
- T-010-03 em andamento: Início, Biblioteca, Microfone, Player, Resultado, Ranking navegáveis e ligados ao `ipc.ts` real; erros do backend aparecem como estado de erro com "Tentar de novo".
- Pendente: componente `List` (não houve consumidor além da Biblioteca, que usa `<ul>`); navegação espacial por setas para controle remoto de TV; 60 FPS não medido; contraste verificado por cálculo dos tokens, não por ferramenta.
- Tarefas review → done (Agente 0, 2026-11-19): integrado em main @ af7bd2c; typecheck, vitest 105/105, vite build, clippy --workspace e cargo test --workspace verdes.

2026-11-19 (Agente 4):
- T-010-05 em review: `apps/desktop/e2e/a11y.e2e.ts` (axe-core, teclado, 4 viewports, movimento reduzido) nas 7 telas/estados existentes. Relatório e achados A11Y-01..05 em [a11y-audit.md](../docs/testing/evidence/a11y-audit.md). Abertos para o Agente 3: A11Y-01 (badge 12 px) e A11Y-02 (axe serious `label-content-name-mismatch`), marcados com `test.fail` até serem corrigidos.
- T-010-05, T-010-03 done (Agente 0, 2026-11-19): integrado em main @ 52a8b95; typecheck, vitest 158/158, Playwright 43/43 + test-song 1/1, cargo test -p game 16 verdes.
- T-010-04 em review (Agente 3, 2026-11-19). Telas sociais: ver SPEC-008 (T-008-03).
  - Ajustes (`Settings.tsx`, item "Ajustes" na NavBar): dificuldade, painel do player, Modo TV, reduzir movimento (sistema/sempre/nunca, aplicado na hora), subpastas, verificar atualizações ao abrir, "Verificar agora". Atualização otimista: se o Rust recusar, volta o valor e mostra o erro.
  - Modo TV (A11Y-03): `html[data-tv=on]` muda a raiz para `clamp(20px, 0.95vw + 10px, 40px)`, o que dá text-sm ≈ 24,7 px em 1080p.
  - Consentimento (SPEC-001 R2): cartão no Início enquanto `autoCheckUpdates=false` e a pergunta ainda não foi feita.
  - ❓ Modo TV e "pergunta de atualização já feita" ficam no localStorage do WebView (`src/display.ts`, marcado `ponytail:`). Proposto ao coordenador como campos de `AppSettings`.
  - Testes: `Settings.test.tsx` (8: ajustes, aplicação imediata, reversão em erro, consentimento ×2, Atualizações ×3).
- T-010-04 done (Agente 0, 2026-11-19): integrado em main @ b3ed719; vitest 173/173, Playwright 44/44. Modo TV e consentimento migram do localStorage para AppSettings (tvMode, updateConsentAsked), contrato em main.
- T-010-05, rodada 2 (Agente 4, 2026-11-19): telas Jogar, Participantes, Nova partida, Partida, Histórico, Referência e diálogos Encerrar, Qual trilha e Pular trecho. Zero violações axe; teclado, viewports e movimento ✅. A11Y-05 coberto por `e2e/spatial.e2e.ts` (5 casos ✅). Novo achado baixo: A11Y-06 (checkbox/rádio de 24 px no desenho). Detalhes em a11y-audit.md.
- T-010-06 em review (Agente 3, 2026-11-19): `src/screens/SongDetails.tsx`, rota `song`.
  - Na Biblioteca, o título do card abre os Detalhes (nome acessível "Detalhes de {título} {artista}"). O "Cantar" virou botão visível e continua direto para o Player, com o mesmo nome acessível de antes ("Cantar {título} {artista ou arquivo}"), então o E2E segue valendo.
  - Conteúdo: título e artista editáveis (`updateSong`, "Salvar" só com mudança válida); arquivo, formato, duração e tamanho; reprodução (compatível / não verificado / a mesma mensagem de codec do Player com a sugestão MP4 H.264/AAC, e "Cantar" bloqueado); referência com selo, explicação e atalho "Importar" ou "Abrir referência".
  - Recordes na dificuldade atual (`getLeaderboard`): recorde da música, top 5 (`RankingTable`) e "Recorde pessoal de" (seletor de cantor; melhor linha dele no ranking, que é top 50). Erro nos recordes não derruba a tela.
  - Botões "Cantar" (com foco inicial) e "Ranking completo".
  - "Suas apresentações" (SPEC-007 R6): `listPerformances({songId, limit: 10})`, com nota, cantor (ou "Anônimo"), dificuldade, data e os selos "Treino" (`competitive=false`) e "Versão anterior" (`isCurrentKey=false`). Se o command falhar, aparece um aviso discreto com a mensagem real e nenhuma linha.
  - Cards da Biblioteca: os botões quebram linha (overflow de 9 px em 960×600, achado pelo E2E de viewports e corrigido).
  - Testes: `SongDetails.test.tsx` (8) e `Library.test` atualizado.
  - Também: fim do fallback localStorage; `tvMode`/`updateConsentAsked` vêm só de `AppSettings` (Rust @ dcdfa0f). Em Ajustes, `<html>` segue o valor salvo, então um valor recusado é desfeito na tela. Atualizações sem plugin (build sem pubkey: "plugin updater not found") mostram "Atualizações automáticas ainda não estão configuradas nesta versão."
  - vitest: `maxWorkers: 1`. Com 2 workers, um não subiu no limite fixo de 60 s com a CPU a 88–100%. `isolate: false` (22 s) foi testado e descartado: vaza estado entre arquivos. Duas rodadas sob carga: 18/187, exit 0 (113 s e 316 s).
  - ❓ Não há "usuário atual" no app, então o recorde pessoal é por cantor escolhido e se limita ao top 50 do ranking.

- T-010-06 done (Agente 0, 2026-11-19): integrado; vitest 189/189, Playwright completo verde. As 13 telas obrigatórias existem.
- A11Y-06 corrigido (Agente 3, 2026-11-19): em Modo TV, `html[data-tv=on] input:is([type=checkbox],[type=radio])` desenha os controles com 2rem, ≥ 40 px em 1080p. A regra fica fora de layer para vencer a utility `size-6`. Fora do Modo TV, nada muda: o alvo continua sendo a linha/label inteira, ≥ 48 px. ⏳ Medida visual com o E2E do Agente 4.
- T-010-06, ajuste (Agente 3, 2026-11-19): "Recorde pessoal de" usa `listPerformances({songId, participantId, limit: 500})`, sem o limite do top 50. `personalBest` (pura) é a maior nota com `competitive && isCurrentKey` na dificuldade da tela, ou "ainda não tem pontuação valendo ranking". "Suas apresentações" já consome o `list_performances` real (main @ df79fe1). Testes: `SongDetails.test.tsx` (9, incluindo `personalBest`).

