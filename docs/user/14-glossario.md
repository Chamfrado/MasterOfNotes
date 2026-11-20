# 14. Glossário

[← Problemas e FAQ](13-problemas-e-faq.md) · [Índice](README.md)

**Afinação** — O quanto a nota que você canta é igual à nota da melodia. Vale 65% da nota final. Veja [Como a pontuação funciona](07-pontuacao.md#afinação-e-os-cents).

**Anônimo** — Nome que aparece no ranking quando ninguém escolheu um cantor (ao cantar pela Biblioteca, por exemplo).

**Calibração de ruído** — Medida de 3 segundos do barulho do ambiente, feita em silêncio, para separar a sua voz do fundo. Veja [Microfone](04-microfone.md#3-calibração-de-ruído).

**Cantada** — Referência melódica criada com [Gravar cantando](05-referencia-melodica.md#gravar-cantando): alguém cantou a melodia e o app anotou as notas. A voz não é gravada.

**Cents** — Unidade para medir pequenas diferenças de altura entre notas. 100 cents = meio tom (de uma tecla do piano para a vizinha). 1200 cents = uma oitava.

**Cobertura** — Fração das notas da música que você acertou. É informativa: não entra na nota final.

**Dificuldade** — Fácil, Normal ou Difícil: quanto a afinação pode desviar e ainda contar como acerto. O ranking é separado por dificuldade.

**Experimental** — Referência melódica importada, gravada cantando ou gerada do vídeo, mas com alinhamento ainda não conferido. Partidas com ela são treino.

**Extraída** — Referência melódica criada com [Gerar automaticamente](05-referencia-melodica.md#gerar-automaticamente-a-partir-do-vídeo): o app separou a voz do vídeo e anotou as notas. Pode ter erros; revise antes de validar.

**Latência** — O pequeno atraso entre o som acontecer e o computador ouvi-lo (ou tocá-lo). O app compensa com o **Ajuste de latência**. Veja [Microfone](04-microfone.md#5-latência-atraso).

**MIDI** — Formato de arquivo que descreve música como notas (qual nota, quando, quanto tempo), sem som gravado. Usado para importar a referência melódica.

**Modelo de separação** — Arquivo `.onnx`, feito por outros autores, que o app usa para separar a voz da música do vídeo em [Gerar automaticamente](05-referencia-melodica.md#gerar-automaticamente-a-partir-do-vídeo). Você o baixa da fonte original; o app não o distribui.

**mon-melody** — Formato JSON próprio do Master Of Notes para a referência melódica.

<a id="nome-das-notas"></a>
**Nome das notas** — O app usa os nomes Dó, Ré, Mi, Fá, Sol, Lá e Si. O número depois do nome é a **oitava**: Dó4 é o Dó do meio do piano, e Dó5 fica uma oitava acima. O sinal **#** quer dizer sustenido, meio tom acima: Lá#4 fica entre Lá4 e Si4. Muitos programas de música e arquivos MIDI usam letras no lugar dos nomes:

| Letra | C | D | E | F | G | A | B |
|---|---|---|---|---|---|---|---|
| Nota | Dó | Ré | Mi | Fá | Sol | Lá | Si |

**Offset** — Deslocamento, em milissegundos, que alinha a referência melódica com o vídeo. Positivo atrasa as notas; negativo adianta. Veja [Referência melódica](05-referencia-melodica.md#alinhar-com-o-vídeo).

**Oitava** — A mesma nota, mais aguda ou mais grave (o Dó seguinte é uma oitava acima). Cantar uma oitava acima ou abaixo vale igual na pontuação.

**Referência melódica** — A lista das notas que devem ser cantadas na música, com início e duração. Sem ela não há pontuação. Veja [Referência melódica](05-referencia-melodica.md).

**Revisão** — Número que aumenta cada vez que o offset de uma referência é salvo.

**Ritmo** — O quanto você começou cada nota na hora certa. Vale 20% da nota final.

**Rodada** — Nas partidas, uma vez em que um cantor canta uma música.

**Sustentação** — O quanto você segurou cada nota afinado até o fim. Vale 15% da nota final.

**Teste de vazamento** — Teste de 10 segundos que confere se o microfone está captando a música das caixas de som. Precisa estar aprovado para valer ranking. Veja [Microfone](04-microfone.md#teste-de-vazamento).

**Trilha (MIDI)** — Cada "instrumento" dentro de um arquivo MIDI (voz, bateria, baixo…). Na importação, você escolhe a trilha da voz.

**Treino** — Apresentação que recebe nota, mas não entra no ranking. Veja [Valendo ranking ou treino?](06-cantar.md#valendo-ranking-ou-treino).

**UltraStar** — Formato de karaokê em texto (`.txt`), criado pela comunidade do jogo UltraStar, com as notas e a letra da música. Pode ser importado como referência; depois, alinhe com o vídeo. Veja [Arquivos UltraStar](05-referencia-melodica.md#arquivos-ultrastar-txt).

**Validada** — Referência melódica cujo alinhamento foi conferido. Libera o ranking.

**Vazamento** — Quando o som da música sai pelas caixas e entra no microfone. Resolve-se com fones de ouvido ou volume mais baixo.

**Versão anterior** — Selo das notas feitas com outra referência, outra revisão ou outra versão do cálculo. Ficam no histórico, fora do ranking atual. Veja [Ranking e histórico](09-ranking-e-historico.md#versão-anterior).

---

[← Problemas e FAQ](13-problemas-e-faq.md) · [Índice](README.md)
