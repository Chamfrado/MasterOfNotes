# Validação do fluxo vertical no app real

Roteiro manual do proprietário. A música de teste vem antes porque é o pré-requisito; o roteiro em si está na seção **Roteiro do proprietário**, ao final.

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
- **`apps/desktop/e2e/test-song.e2e.ts`** (`E2E_PORT=14N5 pnpm --filter desktop exec playwright test e2e/test-song.e2e.ts --no-deps`): gera os três arquivos e confere, no Chromium:
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
- **Gere com a máquina ociosa.** A gravação é em tempo real. Sob carga de CPU ela perde quadros de vídeo (medido: 16 em vez de ~27 por segundo, e quadro velho após o seek). No CI, `test-song.e2e.ts` roda sozinho, depois dos outros testes, num projeto Playwright próprio; para rodá-lo isolado, use `--no-deps`.
- O WebM não tem índice de seek (Cues). O seek funciona porque há um keyframe por segundo; o seek pode ser menos preciso que num MP4 fechado.
- Gerado e verificado no Chromium headless. A reprodução no **WebView2** real (mesmo motor Blink) entra no roteiro manual / `codecs.md`.

## Roteiro do proprietário (fluxo vertical, AC-002 a AC-013)

Leva cerca de 25 min. Preencha a coluna **Resultado** (✅ / ❌ + observação) e anote data, commit (`git log -1 --oneline`), microfone usado e se usou fones ou caixas. Qualquer ❌ vira bug com o passo exato.

### Preparação (uma vez)
1. **Microfone liberado:** Configurações do Windows → Privacidade e segurança → Microfone → "Permitir que aplicativos da área de trabalho acessem o microfone" **ligado**; e o microfone não está mudo (no Painel de Som, a barra de nível reage à voz). *Motivo: nos testes automáticos, o Realtek padrão mediu −105 dBFS, sinal de mudo ou bloqueado.*
2. Gere a música de teste (seção acima) numa pasta fora do repositório, ex.: `C:/Karaoke/teste`. Se quiser, coloque também na pasta 1–2 vídeos **seus** (mp4/webm/mkv/mov/avi) para o teste de formatos.
3. Abra o app: na raiz do repositório, `pnpm dev` (a primeira compilação leva alguns minutos).

### Passos

| # | AC | Passo | Esperado | Resultado |
|---|---|---|---|---|
| 1 | AC-002 | Biblioteca → escolher a pasta `C:/Karaoke/teste` | As 2 músicas de teste (e os seus vídeos) aparecem; o `.json` não aparece | |
| 2 | AC-003 | Buscar "karaoke", depois "TESTE" e depois "xyz" | Filtra sem distinguir maiúsculas e acentos; "xyz" mostra o estado vazio | |
| 3 | AC-002 | Com o app aberto, copie mais um vídeo para a pasta; depois apague-o | Ele aparece e some sozinho em ~2 s | |
| 4 | AC-004 | Abrir `mon-test-song` (com guia) → Cantar → tocar | Vídeo e som sincronizados; a letra na faixa inferior não fica coberta pelo overlay; Espaço pausa e retoma; F = tela cheia; H alterna o overlay; ↑/↓ volume | |
| 5 | AC-004 | Abrir cada vídeo seu (se houver) | Toca, ou mostra a mensagem clara de formato não suportado (AVI deve mostrar a mensagem) | |
| 6 | AC-005 | Microfone → escolher o dispositivo | O medidor reage à voz; "Voz detectada" e a nota aparecem ao cantar um "aaa" | |
| 7 | AC-006 | Microfone → Calibrar (3 s em silêncio) | Mostra "Ambiente bom para cantar." ou um aviso compreensível | |
| 8 | AC-007 | Biblioteca → `mon-test-song` → Referência → importar `mon-test-song.mon-melody.json` | 25 notas desenhadas sobre o vídeo, alinhadas ao "Sol4", "Lá4"… mostrados no vídeo; **Validar alinhamento** | |
| 9 | AC-007 | Repetir o passo 8 para `mon-test-song-karaoke` | Referência validada | |
| 10 | A2 | Microfone → **Teste de vazamento** com **caixas de som** (sem fones), em silêncio | Provavelmente **reprovado** (a guia vaza): é o comportamento correto | |
| 11 | A2 | Repetir o teste de vazamento **com fones** | **Aprovado** | |
| 12 | AC-008/009 | `mon-test-song-karaoke`, **em silêncio** do começo ao fim | Feedback "Nota não detectada"; score ≈ 0 | |
| 13 | AC-008/009 | `mon-test-song-karaoke`, **cantando junto** (cante "lá" na altura das notas mostradas; ouça antes a versão com guia) | Feedback "Bom"/"Muito bom" nas notas acertadas; score coerente com o que você sentiu. Anote o número | |
| 14 | AC-008 | Repetir **cantando propositalmente meio tom acima** em tudo | Score bem menor que no passo 13 | |
| 15 | sync | No passo 13, os acertos aparecem "atrasados"? | Se sim, ajuste a latência no Microfone (±10 ms) e repita; anote o valor que ficou melhor | |
| 16 | AC-010/011 | Depois do passo 13: Resultado → Ranking da música | A performance aparece com seu nome; recorde pessoal e da música corretos | |
| 17 | AC-012 | Jogar → Festa: 2 participantes, 2 músicas de teste (validadas), ordem sorteada; cante as rodadas (pode ser curto) | Ranking parcial com total e média; vencedor ao final | |
| 18 | AC-012 | Durante a festa, feche o app no meio e reabra → Jogar → "Continuar de onde parou" | A festa retoma na rodada certa | |
| 19 | AC-013 | Feche e reabra o app | Pasta, configurações, referências validadas, ranking e histórico preservados | |

### Registro
- Data / commit:
- Microfone / fones ou caixas:
- Bugs encontrados (passo → o que aconteceu):
