# 12. Privacidade

[← Atualizações](11-atualizacoes.md) · [Índice](README.md) · [Próximo: Problemas e FAQ →](13-problemas-e-faq.md)

## Resumo

- **O áudio do microfone nunca é gravado.** Ele é analisado na hora, dentro do seu computador, para descobrir a nota que você canta, e descartado em seguida.
- **Tudo fica no seu computador.** Não há conta, cadastro, nuvem nem envio de dados de uso.
- **A internet só é usada para procurar atualizações**, e só se você permitir.

## O microfone

Enquanto você canta (e nas telas de teste do microfone), o app lê o som do microfone em pequenos pedaços e calcula a altura da nota e o volume. Só esses números são usados, e só durante a música. Ao final, ficam guardados apenas **os resultados**: a nota, as partes da nota (afinação, ritmo, sustentação), a data e o cantor.

Não existe opção de gravar a sua voz, e nenhum arquivo de áudio é criado.

## O que fica salvo e onde

Tudo fica em `%APPDATA%\com.masterofnotes.karaoke\`, num banco de dados local (`masterofnotes.db`):

- o caminho da pasta da biblioteca e a lista de vídeos (nome do arquivo, título, artista, duração);
- as referências melódicas que você importou;
- os nomes dos cantores cadastrados;
- as notas de cada apresentação, com data, dificuldade e versão do cálculo;
- as partidas (Festa, Duelo, Desafio, Solo);
- os seus ajustes (microfone escolhido, calibração, latência, volume, dificuldade etc.).

Seus **vídeos não são copiados**: o app guarda só o caminho deles.

O app também mantém um **registro técnico** (log) em `%LOCALAPPDATA%\com.masterofnotes.karaoke\logs\`, para ajudar a entender erros. Ele não contém áudio.

Para apagar tudo, desinstale o app e apague a pasta `%APPDATA%\com.masterofnotes.karaoke\` (veja [Desinstalar](01-instalacao.md#desinstalar)).

## Internet

O único acesso à internet é a **verificação de atualizações**, na página de versões do projeto no GitHub:

- automática ao abrir o app, **só se você ligar** **Verificar atualizações ao abrir** (vem desligado);
- ou quando você clica em **Verificar agora**.

Nessa consulta, como em qualquer acesso a um site, o servidor do GitHub recebe dados técnicos da conexão, como o endereço de rede do seu computador. O app não envia nada sobre você, suas músicas ou suas notas.

Sem internet, tudo funciona normalmente.

## Direitos das músicas

O Master Of Notes não vem com músicas, vídeos, letras ou referências. Você é responsável pelos direitos do conteúdo que usar.

---

[← Atualizações](11-atualizacoes.md) · [Índice](README.md) · [Próximo: Problemas e FAQ →](13-problemas-e-faq.md)
