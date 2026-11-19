# 6. Cantar

[← Referência melódica](05-referencia-melodica.md) · [Índice](README.md) · [Próximo: Como a pontuação funciona →](07-pontuacao.md)

## Abrir o player

Há três caminhos:

- **Biblioteca → Cantar**, no cartão da música;
- **Cantar**, no topo da tela da música;
- **Cantar**, na rodada atual de uma partida (veja [Modos de jogo](08-modos-de-jogo.md)).

Cantando pela Biblioteca, a apresentação fica **sem nome de cantor** e aparece como *Anônimo* no ranking. Para registrar seu nome, use o modo **Solo** em **Jogar** (veja [Modos de jogo](08-modos-de-jogo.md#solo)).

O player ocupa a janela toda (o menu some). Aparece o nome da música e o botão **Começar**. Clique nele: vem a contagem **3, 2, 1** e o vídeo começa.

![Animação: na Biblioteca, clique em Cantar; o player mostra o título e o botão Começar; contagem 3-2-1; o vídeo começa com o painel de afinação e a pontuação no alto.](media/cantar-comecar.gif)

*Animação com música de teste própria do projeto e notas ilustrativas.* Vídeo opcional do fluxo completo, do Começar ao Resultado: [cantar-fluxo-completo.webm](media/cantar-fluxo-completo.webm) (o GitHub não toca o vídeo na página; baixe ou abra o arquivo).

## Durante a música

![Player durante a música: no alto à esquerda, o painel com a nota alvo Ré4, o selo Excelente e a bolinha de afinação; no alto à direita, a pontuação parcial 81,1; embaixo, a barra de controles.](media/cantar-painel-completo.png)

*Notas e pontuação ilustrativas.*

### O painel de feedback

No alto da tela:

- **À esquerda, a nota que você deve cantar** (por exemplo, *Ré4*; veja [nome das notas](14-glossario.md#nome-das-notas)) e uma avaliação dos últimos instantes: **Excelente**, **Muito bom**, **Bom**, **Precisa melhorar** ou **Não detectado** (o app não ouviu voz).
- **A bolinha** mostra sua afinação: no centro = afinado; à direita = você está **acima** da nota (mais agudo); à esquerda = **abaixo** (mais grave).
- **À direita, a pontuação parcial**, que considera só as notas que já terminaram. Se a partida for treino, aparece o selo **Treino**.
- Nos trechos sem canto (introdução, solos), aparece *"Aguardando a próxima nota…"*. Esses trechos não contam para a nota.

A parte de baixo da tela fica livre para a letra do vídeo.

O painel tem três modos. Troque com a tecla **H** ou o botão **Painel**:

| Modo | Mostra |
|---|---|
| **Completo** | Nota, afinação e pontuação |
| **Compacto** | Só a pontuação |
| **Oculto** | Nada |

![Animação: a tecla H alterna o painel entre completo, compacto e oculto.](media/cantar-painel-modos.gif)

O modo escolhido fica salvo. Ele também pode ser escolhido em [Ajustes](10-ajustes-e-modo-tv.md).

### Controles

Mexa o mouse ou aperte uma tecla para mostrar os controles. Eles somem sozinhos depois de 3 segundos.

![Barra de controles: tempo, barra de posição, botão Continuar, Volume, Painel: completo (H), Tela cheia (F) e Sair (Esc).](media/cantar-controles.png)

| Controle | Tecla | O que faz |
|---|---|---|
| **Pausar** / **Continuar** | Espaço | Pausa e retoma. Pausar não tira a partida do ranking. |
| Barra de posição | ← / → | Volta ou avança 5 segundos. Numa partida que vale ranking, pede confirmação (veja abaixo). |
| **Volume** | ↑ / ↓ | Sobe ou desce 5%. Fica salvo. |
| **Painel** | H | Troca o modo do painel. |
| **Tela cheia** | F | Liga e desliga a tela cheia. |
| **Sair** | Esc | Sai do player. **A partida é descartada: nada é salvo.** |

### Pular trecho = treino

Avançar ou voltar a música numa partida que vale ranking abre a pergunta **Pular trecho?**:

![Janela "Pular trecho?": Avançar ou voltar a música transforma esta partida em treino: a nota não entra no ranking. Botões Continuar valendo e Pular e virar treino.](media/cantar-pular-trecho.png)

- **Continuar valendo**: não pula, e a partida continua valendo.
- **Pular e virar treino**: pula, e a partida passa a ser treino. Aparece o selo **Treino — fora do ranking**.

## Valendo ranking ou treino?

Uma partida **vale ranking** quando:

1. a música tem **referência validada** ([Referência melódica](05-referencia-melodica.md#validar));
2. o **teste de vazamento** está aprovado para o microfone e o volume atuais ([Microfone](04-microfone.md#teste-de-vazamento));
3. você **não pula trechos** durante a música.

Senão, ela é **treino**: você recebe a nota, ela fica no histórico com o selo **Treino**, mas não entra no ranking. Com a música pausada, o player mostra o motivo:

| Motivo mostrado | Como resolver |
|---|---|
| *Faça o teste de vazamento do microfone (ou use fones) para pontuar no ranking.* | Faça o [teste de vazamento](04-microfone.md#teste-de-vazamento). |
| *A referência desta música ainda não foi validada: a apresentação é só treino.* | [Valide o alinhamento](05-referencia-melodica.md#validar). |
| *A referência desta música foi marcada como inválida: a apresentação é só treino.* | [Importe outra referência](05-referencia-melodica.md#importar). |
| *Esta música não tem referência melódica: não há pontuação, só métricas de treino.* | [Importe uma referência](05-referencia-melodica.md#importar). |

Se a música tem referência validada mas o teste de vazamento não foi feito (ou precisa ser refeito), o player avisa antes de começar e troca o botão para **Começar como treino**. O botão **Fazer teste** leva à tela do Microfone.

![Antes de começar: aviso "Para valer ranking, faça antes o teste de vazamento do microfone.", botão Fazer teste e botão Começar como treino.](media/cantar-treino-vazamento.png)

### Aviso: o microfone está captando a música

Se o app ouvir som no microfone nos trechos em que ninguém deveria estar cantando, aparece:

![Aviso no player: "O microfone está captando a música — use fones de ouvido."](media/cantar-aviso-vazamento.png)

Use fones de ouvido ou baixe o volume.

## Resultado

Quando o vídeo termina, aparece a tela **Resultado**.

![Tela Resultado: nota 87,4, "Novo recorde pessoal!", Afinação 91%, Ritmo 80%, Sustentação 86%, Cobertura 90%, "36 de 40 notas acertadas · recorde da música: 92,1" e os botões Cantar de novo, Ver ranking e Outra música.](media/resultado.png)

*Valores ilustrativos.*

- **A nota**, de 0 a 100. Em treino, aparece o selo **Treino** embaixo dela.
- **Novo recorde pessoal!** quando você iguala ou supera a sua melhor nota nessa música.
- **Afinação**, **Ritmo**, **Sustentação** e **Cobertura**, em porcentagem. O que cada uma significa está em [Como a pontuação funciona](07-pontuacao.md).
- Quantas notas você acertou e o recorde da música.
- Botões: **Cantar de novo**, **Ver ranking** e **Outra música**. Numa partida com rodadas, o botão principal é **Voltar para a partida**.

### Música sem referência

Sem referência não há nota. O Resultado mostra só **Métricas exploratórias (fora do ranking)**:

- **Tempo cantando**: em quanto da música o app ouviu voz;
- **Estabilidade da afinação**: o quanto sua voz ficou firme, sem oscilar, em cada nota que você sustentou.

![Resultado sem referência: aviso de que não há pontuação, botão Importar referência e as métricas Tempo cantando 72% e Estabilidade da afinação 64%.](media/resultado-sem-referencia.png)

*Valores ilustrativos.*

---

[← Referência melódica](05-referencia-melodica.md) · [Índice](README.md) · [Próximo: Como a pontuação funciona →](07-pontuacao.md)
