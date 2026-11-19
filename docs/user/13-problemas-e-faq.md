# 13. Solução de problemas e perguntas frequentes

[← Privacidade](12-privacidade.md) · [Índice](README.md) · [Próximo: Glossário →](14-glossario.md)

Quando algo dá errado, o app mostra uma mensagem em português e, quando faz sentido, um botão **Tentar de novo**. Erros nunca apagam seus dados.

## Vídeos e biblioteca

### O vídeo não toca / aparece "Formato incompatível"

O Windows não consegue tocar a compressão desse arquivo dentro do app. Converta para **MP4 com vídeo H.264 e áudio AAC** e coloque o arquivo convertido na pasta da biblioteca. Veja [Formatos de vídeo](03-biblioteca.md#formatos-de-vídeo).

### Uma música não aparece na Biblioteca

- O app só lista **MP4, MKV, WEBM, MOV e AVI**. Arquivos de áudio (MP3, WAV…) não entram.
- O arquivo está numa **subpasta**? Ligue **Procurar também nas subpastas** em [Ajustes](10-ajustes-e-modo-tv.md) e clique em **Atualizar** na Biblioteca.
- Você acabou de copiar o arquivo? Clique em **Atualizar**.
- O arquivo foi tirado da pasta? Músicas que somem da pasta saem da lista, mas o histórico delas fica guardado e volta se o arquivo voltar.

### Renomeei ou mudei um vídeo de lugar. Perdi o ranking?

Não, desde que ele continue dentro da pasta da biblioteca. O app reconhece o vídeo pelo conteúdo, não pelo nome. O título só acompanha o novo nome do arquivo se você nunca editou o título.

### A busca não encontra a música

A busca procura no título, no artista e no nome do arquivo. Confira se o título foi editado (clique em **Editar** na música).

## Microfone

### "Nenhum microfone encontrado"

1. Conecte o microfone e volte à tela **Microfone**.
2. Confira se o Windows permite o acesso: **Configurações → Privacidade e segurança → Microfone**, com o acesso ao microfone ligado e **Permitir que aplicativos da área de trabalho acessem o microfone** ligado.
3. Teste o microfone em **Configurações → Sistema → Som**.

### O selo "Voz detectada" não acende

- Escolha o microfone certo em **Dispositivo de entrada**.
- Faça a **Calibração de ruído** em silêncio. Se calibrou com barulho, a voz pode ficar abaixo da linha do ruído.
- Cante mais perto do microfone ou aumente o volume de entrada no Windows.
- Veja [Microfone](04-microfone.md).

### A pontuação parece atrasada (ou adiantada) em relação ao que eu canto

Ajuste a **latência**: use **Medir latência** ou o controle **Ajuste de latência**. Veja [Latência](04-microfone.md#5-latência-atraso).

### O teste de vazamento reprova

O microfone está ouvindo a música das caixas de som. **Use fones de ouvido** ou **baixe o volume da música** e teste de novo. Veja [Teste de vazamento](04-microfone.md#teste-de-vazamento).

### Aparece "O microfone está captando a música — use fones de ouvido."

O mesmo problema, detectado durante a música. Use fones ou baixe o volume.

### Posso cantar em dupla, com dois microfones ao mesmo tempo?

Não. O app ouve um microfone e uma voz por vez. No **Duelo**, cada pessoa canta na sua vez.

## Pontuação e ranking

### Minha partida virou treino

A tela Resultado mostra o motivo, e a tabela completa está em [Quando a partida valeu como treino](06-cantar.md#quando-a-partida-valeu-como-treino). Os motivos mais comuns: referência não validada, teste de vazamento não feito (ou volume aumentado depois do teste), trecho pulado, falhas no áudio do microfone e menos da metade da música tocada.

### Não aparece pontuação, só "Métricas exploratórias"

A música não tem referência melódica. Veja [Referência melódica](05-referencia-melodica.md).

### A música não aparece para escolher na Festa, no Duelo ou no Desafio

Esses modos só mostram músicas com **referência validada** e que tocam neste computador. Valide a referência da música. Veja [Modos de jogo](08-modos-de-jogo.md).

### Minha nota não aparece no ranking

- O ranking é **por dificuldade**: confira se está olhando a mesma dificuldade em que você cantou.
- Apresentações de **treino** não entram.
- Se a referência ou o cálculo mudaram depois, a nota virou **Versão anterior**. Veja [Versão anterior](09-ranking-e-historico.md#versão-anterior).

### Cantei uma oitava abaixo do cantor. Perdi pontos?

Não. Oitava acima ou abaixo vale igual. Veja [Como a pontuação funciona](07-pontuacao.md#regras-que-muita-gente-pergunta).

### Cantar mais alto dá mais pontos?

Não. O volume não entra na conta.

### Saí do player no meio da música. A nota foi salva?

Não. Sair (**Esc** ou **Sair**) descarta a apresentação. Numa partida, a rodada continua pendente.

## Referência melódica

| Mensagem | O que significa |
|---|---|
| *o arquivo MIDI tem várias trilhas com notas; escolha a trilha vocal* | Escolha a trilha da voz na janela **Qual trilha é a voz?**. |
| *formato não suportado: MIDI com timecode SMPTE* / *MIDI tipo 2* | Esse tipo de MIDI não é aceito. Tente exportar o MIDI de novo como "tipo 1" num editor de MIDI. |
| *formato não reconhecido (esperado "mon-melody")* | O JSON não está no formato do Master Of Notes. |
| *a referência não tem notas* | O arquivo (ou a trilha escolhida) está vazio. |
| *nota nº N: altura MIDI … fora da faixa 24–108* | Uma nota está grave ou aguda demais para ser cantada; confira o arquivo. |

## Geral

### "Não foi possível verificar ou baixar a atualização"

Sem internet, essa mensagem é normal. Durante as versões de testes (pré-releases), como a 0.1.1, ela também pode aparecer com internet. Isso é esperado: confira as novidades na [página de releases](https://github.com/Chamfrado/MasterOfNotes/releases). O app continua funcionando normalmente. Veja [Atualizações](11-atualizacoes.md#se-der-erro).

### Funciona sem internet?

Sim, tudo. A internet só é usada para procurar atualizações, se você permitir.

### Funciona no Mac ou no Linux?

Por enquanto, só no Windows.

### Como faço backup dos meus dados?

Feche o app e copie a pasta `%APPDATA%\com.masterofnotes.karaoke\`. Para restaurar, feche o app e coloque a pasta de volta. Seus vídeos ficam onde você os guardou; faça backup deles à parte.

### "Não foi possível abrir o banco de dados em …. Nenhum dado foi apagado."

O app não conseguiu abrir o arquivo de dados. Feche e abra o app de novo. Se continuar, confira se há espaço livre no disco e se a pasta indicada na mensagem existe. Seus dados não foram apagados.

### O app não abre ou mostra uma tela em branco

Reinstale o app (seus dados são mantidos). Se não resolver, pode faltar o componente **Microsoft Edge WebView2**; instale-o pelo site da Microsoft.

### Onde peço ajuda ou relato um problema?

Na página do projeto: <https://github.com/Chamfrado/MasterOfNotes/issues>. Se possível, anexe o registro técnico de `%LOCALAPPDATA%\com.masterofnotes.karaoke\logs\`.

---

[← Privacidade](12-privacidade.md) · [Índice](README.md) · [Próximo: Glossário →](14-glossario.md)
