# 7. Como a pontuação funciona

[← Cantar](06-cantar.md) · [Índice](README.md) · [Próximo: Modos de jogo →](08-modos-de-jogo.md)

A nota vai de **0 a 100** e mede o quanto você cantou **as notas certas, na hora certa, segurando até o fim**. Não importa se a sua voz é bonita, forte ou parecida com a do cantor original. Esta página explica a regra atual do cálculo.

## O que o app ouve

O app ouve o microfone dezenas de vezes por segundo e descobre **qual nota você está cantando**. Ao mesmo tempo, sabe pela [referência melódica](05-referencia-melodica.md) qual nota deveria estar soando naquele instante. A pontuação compara as duas.

O som do microfone é analisado na hora e descartado; nada é gravado. Veja [Privacidade](12-privacidade.md).

## As três partes da nota

| Parte | Peso | Pergunta que responde |
|---|---|---|
| **Afinação** | **65%** | Você cantou a nota certa? |
| **Ritmo** | **20%** | Você entrou na hora certa? |
| **Sustentação** | **15%** | Você segurou a nota até o fim, afinado? |

**Nota final = 65% da afinação + 20% do ritmo + 15% da sustentação.** No Resultado, cada parte aparece em porcentagem.

Notas longas pesam mais que notas curtas: errar uma nota de 3 segundos custa mais que errar uma de 0,2 segundo.

### Afinação e os "cents"

Para medir o quanto você desafinou, o app usa **cents**: **100 cents = meio tom**, a distância entre duas teclas vizinhas do piano (de Dó para Dó♯, por exemplo). Uma diferença de 10 cents quase ninguém percebe; 50 cents é "no meio do caminho" entre duas notas.

A cada instante dentro de uma nota:

- se você está **perto o bastante** da nota (dentro da faixa "cheia"), ganha o ponto inteiro daquele instante;
- se está **longe demais** (além da faixa "zero"), não ganha nada;
- **no meio do caminho**, ganha uma parte, proporcional. Não existe degrau: desafinar um pouco mais tira só um pouco mais de ponto.

As faixas dependem da dificuldade, escolhida em [Ajustes](10-ajustes-e-modo-tv.md):

| Dificuldade | Ponto inteiro até | Zero a partir de |
|---|---|---|
| **Fácil** | 40 cents | 200 cents (1 tom) |
| **Normal** | 25 cents | 165 cents |
| **Difícil** | 15 cents | 155 cents |

Exemplo no **Normal**: cantar tudo 20 cents abaixo ainda dá afinação cheia; cantar tudo meio tom (100 cents) acima dá mais ou menos metade da afinação.

### Ritmo

Para cada nota, o app procura o momento em que você **começou a cantá-la afinado** e compara com o início da nota na referência:

- até **80 ms** de diferença (menos de um décimo de segundo), adiantado ou atrasado: ritmo cheio;
- **300 ms** ou mais: ritmo zero;
- entre os dois: parcial.

O ritmo só conta nas notas que você cantou afinado. Entrar na hora certa cantando a nota errada não ganha ponto de ritmo.

Notas muito curtas (menos de 0,12 segundo, comuns em trechos rápidos) aceitam um pequeno atraso extra, de até 30 ms, porque nelas qualquer atraso pesa muito.

Se o ritmo parece sempre baixo mesmo cantando no tempo, provavelmente falta ajustar a [latência](04-microfone.md#5-latência-atraso).

### Sustentação

Mede quanto de cada nota você **segurou afinado do começo ao fim**. Parar no meio, perder o fôlego ou deixar a voz cair no final reduzem a sustentação.

### Cobertura

**Cobertura** é a fração das notas da música que você acertou (cantou afinado em pelo menos metade da nota): *"36 de 40 notas acertadas"* = 90%. Ela é informativa e **não entra na conta da nota**.

## Regras que muita gente pergunta

- **Uma oitava acima ou abaixo vale igual.** Homens, mulheres e crianças cantam a mesma melodia em alturas diferentes. Cantar a nota uma oitava (ou duas) acima ou abaixo conta como afinado, sem bônus nem desconto.
- **Outra tonalidade não vale.** Se a referência está em um tom e você canta tudo em outro (que não seja oitava), é desafinação.
- **Cantar mais alto não aumenta a nota.** O volume da voz não entra em nenhuma conta. Ele só precisa ser suficiente para o app ouvir você acima do ruído do ambiente (veja [Calibração de ruído](04-microfone.md#3-calibração-de-ruído)).
- **Trechos sem canto não contam.** Introdução, solos e pausas previstas na melodia não tiram nem dão pontos.
- **Nota que você não canta conta zero.** Não dá para tirar nota alta cantando só um pedaço da música: cada nota da referência entra na conta, cantada ou não.
- **Vibrato não é desafinação.** O vibrato é aquela oscilação natural da voz numa nota longa. O app compara a altura média da sua voz ao longo de cada oscilação (cerca de 0,2 segundo), e não cada instante isolado, então um vibrato centrado na nota não tira pontos. Nos testes, um vibrato de até ¾ de semitom para cada lado teve nota praticamente cheia. Enfeites curtos entre notas também são absorvidos. Já cantar a nota toda acima ou abaixo continua contando como desafinação.
- **Oscilar em volta da nota não é acertá-la.** Um vibrato largo demais (bem mais que meio tom para cada lado), um trinado ou uma voz que treme de um lado para o outro sem parar na nota conta como desafinação.
- **Gesto de estilo não é erro.** Deixar a voz cair no fim da nota, subir e voltar, ou chegar à nota deslizando de baixo são movimentos únicos de interpretação e tiram poucos pontos. Um enfeite aqui e ali quase não pesa; repetir um gesto grande em todas as notas (por exemplo, deixar a voz cair bastante no fim de cada uma) custa alguns pontos, porque nesses trechos você não está na nota.
- **A pontuação parcial**, mostrada durante a música, considera só as notas que já terminaram.
- **Falha no áudio não é silêncio.** Se o computador perder pedacinhos do som do microfone (por exemplo, um cabo USB com mau contato), esses pedaços não contam contra você. Mas se faltar mais de 5% do áudio da música, a apresentação vira treino, porque a nota não seria confiável.

## O painel ao vivo

As avaliações que aparecem no player (**Excelente**, **Muito bom**, **Bom**, **Precisa melhorar**) resumem a sua afinação nos últimos 0,2 segundo. **Não detectado** significa que o app não ouviu voz nesse intervalo. Elas servem para você se corrigir na hora; a nota final vem do cálculo completo.

## Até onde dá para confiar

- O cálculo foi ajustado e conferido com **testes automáticos**: vozes sintéticas afinadas, desafinadas, atrasadas, com vibrato, com acompanhamento. Com voz humana real, os resultados podem variar com o microfone, o ambiente e a qualidade da referência.
- A regra **pode ser ajustada em versões futuras**. Quando isso acontece, a versão do cálculo muda.
- **Notas de versões diferentes não se misturam no ranking.** Cada nota guarda a versão do cálculo e a revisão da referência com que foi feita. Se uma delas mudar, as notas antigas continuam no histórico com o selo **Versão anterior**, mas o ranking atual só compara notas feitas nas mesmas condições. Veja [Ranking e histórico](09-ranking-e-historico.md#versão-anterior).
- O cálculo não tem sorteio nem aleatoriedade: os mesmos sons, comparados com a mesma referência, dão sempre a mesma nota.

---

[← Cantar](06-cantar.md) · [Índice](README.md) · [Próximo: Modos de jogo →](08-modos-de-jogo.md)
