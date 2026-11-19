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
