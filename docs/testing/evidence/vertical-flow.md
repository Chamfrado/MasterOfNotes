# Validação do fluxo vertical no app real

Roteiro manual do proprietário: será completado pelo coordenador. Este documento começa pela música de teste.

## Música de teste (T-003-05)

Música de autoria própria, gerada em código (Lei 19: sem conteúdo de terceiros, sem ffmpeg). Ela existe para validar no app real a importação, a referência, o player e a pontuação.

### Gerar

Na raiz do repositório (Node ≥ 22.18, que roda `.ts` direto; dependências instaladas com `pnpm install`):

```sh
pnpm --filter desktop exec node e2e/tools/gen-test-song.ts "C:/Karaoke/teste"
```

Leva cerca de 1 minuto, porque a gravação é em tempo real. Ele usa o Chromium do Playwright (`pnpm --filter desktop exec playwright install --only-shell chromium`, se ainda não estiver instalado). **Não versione a saída.** Escolha uma pasta fora do repositório; ela pode ser a própria pasta da biblioteca.

| Arquivo | Conteúdo |
|---|---|
| `mon-test-song.webm` | 31 s, 1280×720, VP8 + Opus. Acompanhamento grave (senoides puras, MIDI ≤ 55) + **melodia-guia** (tom com harmônicos leves, MIDI 60–72). |
| `mon-test-song-karaoke.webm` | Igual, mas **sem** a melodia-guia: para cantar sem vazamento da guia. |
| `mon-test-song.mon-melody.json` | Referência `mon-melody` v1 **exata** das notas da guia (25 notas). |

O vídeo mostra o título "Master Of Notes — teste", o tempo, uma barra de progresso, a nota atual em destaque ("Sol4"), a próxima nota com contagem e, na **faixa inferior central**, a letra com a sílaba atual realçada (como num karaokê real). Isso serve para conferir que o overlay do app não cobre a letra (SPEC-003 R7).

Estrutura da melodia, também verificada em teste:

- 2 s de introdução só instrumental;
- duas notas **repetidas ligadas** (Sol4 em 5,7 s e 6,2 s);
- uma nota **curta** de 150 ms (Lá4 em 6,7 s);
- pausa instrumental de 3,4 s (12,6 s a 16 s);
- salto de quinta (Sol4 para Dó5 em 18,2 s);
- final longo de 2,6 s.

A **fonte da verdade** é `apps/desktop/e2e/tools/test-song.mon-melody.json`. O gerador desenha o vídeo e sintetiza o áudio a partir dela, e copia essa mesma referência para a saída.

### Usar no app (resumo; o coordenador detalha o roteiro)

1. Escolha a pasta de saída como biblioteca. As duas versões aparecem.
2. Na música desejada, abra **Referência** e importe `mon-test-song.mon-melody.json`. Confira o alinhamento e marque como validada.
3. Cante a versão **karaokê** com o microfone. A versão com guia serve para ouvir a melodia e para o teste de vazamento.

### Verificação automatizada

- **`crates/game/tests/test_song.rs`** (`cargo test -p game`): a referência passa pelo `scoring_engine::parse_reference_json` real sem perder nem fundir notas, e tem introdução, par repetido ligado, nota curta, pausa e fim antes de 31 s.
- **`apps/desktop/e2e/test-song.e2e.ts`** (`E2E_PORT=14N5 pnpm --filter desktop exec playwright test e2e/test-song.e2e.ts`): gera os três arquivos e confere, no Chromium:
  - os dois vídeos carregam, têm duração de 31 s (o Duration é injetado no cabeçalho WebM) e tocam;
  - após um seek para 21 s, o quadro exibido é o daquele tempo (pixel da barra de progresso);
  - a referência servida é idêntica à fonte;
  - em cada uma das 25 notas, a altura da referência domina a ±1 semitom (razão > 10) na versão com guia e está ausente na versão karaokê (< 1% da potência da guia);
  - as notas que entram depois de pausa começam a ≤ 30 ms da referência.

Medido em 2026-11-19 (Chromium headless 1.65.1):

| Medida | Valor |
|---|---|
| Duração (com guia / karaokê) | 31,0 s / 31,0 s |
| Seek para 21 s | `currentTime` 21,0; quadro correto |
| Razão altura-alvo / vizinhos | 72 a 961 (mínimo na nota de 150 ms) |
| Vazamento da guia na versão karaokê | ≤ 0,004% da potência da guia |
| Onset das 9 notas após pausa | +16 a +17 ms. O detector (janela de 10 ms, limiar de 50%) atrasa alguns ms por causa da rampa de ataque de 12 ms; o desvio real é menor. |

### Limitações ❓

- Alinhamento de notas **ligadas** não é medido automaticamente (a janela de 10 ms não separa notas vizinhas). Elas usam o mesmo relógio de áudio das notas medidas.
- O WebM não tem índice de seek (Cues). O seek funciona porque há um keyframe por segundo; o seek pode ser menos preciso que num MP4 fechado.
- Gerado e verificado no Chromium headless. A reprodução no **WebView2** real (mesmo motor Blink) entra no roteiro manual / `codecs.md`.
