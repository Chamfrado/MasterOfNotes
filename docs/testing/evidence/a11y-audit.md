# Auditoria de acessibilidade e "10-foot" (T-010-05)

Auditor: Agente 4 · 2026-11-19 · `main` @ 37bfce6 · Chromium headless (Playwright 1.65.1) · axe-core via `@axe-core/playwright` 4.14.
Telas: Início, Biblioteca, Microfone, Player (pronto), Player pausado (barra de controles), Resultado, Ranking. Todas com o mock de IPC só de teste (`e2e/tauri-mock.ts`).
O vídeo do Player e do Resultado é **gerado na página** (canvas + MediaRecorder, WebM de 64×64): não há binário nem conteúdo de terceiros.

**Como reproduzir:** `pnpm --filter desktop exec playwright test e2e/a11y.e2e.ts --reporter=list`. As medidas aparecem nas linhas `[axe]`, `[teclado]` e `[viewport]`. As capturas por viewport vão para `apps/desktop/test-results/`, que não é versionada.
Os achados em aberto estão marcados com `test.fail(...)` e o ID do achado. Quando o achado for corrigido, o teste "passa inesperadamente" e acusa, e aí é só remover a marcação.

## Resultado por critério

| Critério | Resultado |
|---|---|
| (a) axe: zero violações serious/critical (tags wcag2a/aa, 21a/aa, 22aa, best-practice) | ✅ em 6 de 7 telas · ❌ Biblioteca: **A11Y-02**. Nenhuma outra violação, de nenhuma gravidade, em nenhuma tela. |
| (b) Tab alcança todos os elementos, foco visível (outline ≥ 2 px) e dentro da tela, sem armadilha | ✅ 7 de 7 telas (Início 8, Biblioteca 15, Microfone 7, Player 2, Player pausado 6, Resultado 7, Ranking 6 alvos; todos alcançados) |
| (b) Esc volta à tela anterior | ✅ 6 de 6 telas com "anterior" |
| (b) Fluxo completo só com teclado: Início → Biblioteca → Player → Resultado → Ranking | ✅ |
| (c) Sem overflow horizontal em 960×600, 1366×768, 1920×1080 e 3840×2160 | ✅ 7 telas × 4 viewports |
| (c) Alvos interativos ≥ 48 px CSS | ✅ 7 × 4 (0 alvos pequenos) |
| (c) Menor texto ≥ 14 px CSS | ✅ 6 de 7 · ❌ Biblioteca: **A11Y-01** |
| (d) Movimento reduzido pela preferência do sistema (`emulateMedia reduce`, ajuste `system`) | ✅ 7 de 7: nenhuma animação ou transição com duração > 0,01 ms (CSS + `getAnimations()`) |
| (d) Movimento reduzido pelo ajuste do app (`reducedMotion: "on"`, sistema sem preferência) | ✅ 7 de 7 |
| Controle negativo: sem redução, o Início anima | ✅ (prova que a checagem (d) não passa por vacuidade) |

Medidas de fonte. A raiz é `clamp(16px, 0.55vw + 10px, 28px)`, o que dá 16 / 17,5 / 20,6 / 28 px nas quatro viewports. O menor texto fora da Biblioteca é 14 / 15,3 / 18 / 24,5 px (`text-sm`).

## Achados para o Agente 3

| ID | Severidade | Tela | Achado | Medido | Sugestão |
|---|---|---|---|---|---|
| A11Y-01 | Média | Biblioteca | Texto do `Badge` (`text-xs`) abaixo de 14 px | 12 px em 960×600; 13,1 px em 1366×768; 15,4 px em 1920×1080 | `text-sm` no `Badge` (vira 14 / 15,3 / 18 px) |
| A11Y-02 | **Serious** (WCAG 2.5.3, Label in Name) | Biblioteca | O nome acessível do botão de cada música (`aria-label="Cantar {título}, de {artista}"`) não contém o texto visível na ordem em que aparece ("{título} {artista}"). Quem usa controle por voz e diz o que vê não aciona o botão. | axe `label-content-name-mismatch`, 3 nós | Tirar o `aria-label` e pôr um prefixo só para leitor de tela: `<span className="sr-only">Cantar </span>` antes do título. O nome fica "Cantar {título} {artista}". |
| A11Y-03 | Baixa (10-foot) | Todas | A 1920×1080 o menor texto é 18 px (`text-sm`) e o badge 15,4 px. Para leitura a cerca de 3 m de uma TV, a referência usual é ≥ 24 px a 1080p. | ver medidas | Em TV, considerar `text-sm` mínimo de 1rem, ou uma raiz maior no modo TV. Não é critério da SPEC-010; fica como registro. |
| A11Y-04 | Baixa (10-foot) | Player pausado | Os sliders "Posição na música" e "Volume" têm caixa ≥ 48 px, mas o desenho é fino: trilho de ~4 px e polegar de ~12 px a 1080p. Ruim de ver de longe. | captura 1920×1080 | Trilho/polegar maiores, ou realce visual do slider focado além do outline |
| A11Y-05 | Baixa | — | Não há navegação espacial por setas (controle remoto de TV); a SPEC-010 já registra isso como pendente | — | Tarefa futura |

**Os testes do E2E dependem do nome acessível.** Corrigir A11Y-02 muda o nome acessível dos botões "Cantar …". `smoke.e2e.ts` e `a11y.e2e.ts` localizam esses botões pelo nome, então o Agente 4 ajusta os seletores no mesmo ciclo. Avise antes do merge.

## Observações ❓

- O reset de movimento usa `0.01ms !important` (técnica comum que mantém `animationend` disparando). O teste considera ≤ 0,01 ms como "sem animação". Movimento feito em JS (`requestAnimationFrame`) não é coberto por essa checagem; hoje não há nenhum.
- "Foco visível" é medido como outline ≥ 2 px (o projeto usa 3 px `#ffe066`). O contraste do contorno com cada fundo não foi medido por ferramenta; o axe não cobre isso.
- Contraste de cor: o axe verificou o texto renderizado e não achou violação. Estados de hover e de foco não foram verificados.
- Medido no Chromium headless, não no WebView2 real (os dois usam o mesmo motor Blink; renderização de fontes e escala de DPI do Windows podem diferir).
- Não auditados (as telas ainda não existem): Detalhes da música, Referência, Seleção de modo, Participantes, Festa, Histórico, Configurações, Atualizações. Também não foi auditado o diálogo "Pular trecho?" do Player. Recomendo uma nova rodada na T-010-04.
