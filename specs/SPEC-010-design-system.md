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
| T-010-01 | Tailwind v4 + tokens + fontes locais + layout/navegação | Agente 3 | todo |
| T-010-02 | Componentes base + testes de acessibilidade (role/label/foco) | Agente 3 | todo |
| T-010-03 | Telas do fluxo vertical (Início, Biblioteca, Microfone, Player, Resultado, Ranking) | Agente 3 | todo |
| T-010-04 | Telas sociais, Configurações, Atualizações | Agente 3 | todo |

## STATUS
Pendente.
