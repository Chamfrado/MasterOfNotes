# 1. Instalação e desinstalação

[← Índice](README.md) · [Próximo: Primeira execução →](02-primeira-execucao.md)

## O que você precisa

- **Windows 64 bits.** O app foi testado no Windows 11.
- **Microsoft Edge WebView2**, o componente do Windows que desenha a tela do app. No Windows 11 ele já vem instalado.
- **Um microfone.** Pode ser o do notebook, um headset ou um microfone USB.
- **Fones de ouvido (recomendado).** Sem fones, o microfone pode captar a música que sai das caixas de som, e aí a partida não vale ranking. Veja [Teste de vazamento](04-microfone.md#teste-de-vazamento).
- **Seus vídeos de karaokê** numa pasta do computador. O app não vem com músicas.

Não precisa de internet para usar o app nem de conta ou cadastro.

## Instalar

1. Abra a página de versões do projeto: <https://github.com/Chamfrado/MasterOfNotes/releases>.
2. Na versão mais recente, baixe o arquivo que termina em **`_x64-setup.exe`** (por exemplo, `Master Of Notes_0.1.0_x64-setup.exe`).
3. Dê dois cliques no arquivo baixado.
4. Se o Windows mostrar o aviso **"O Windows protegeu o computador"**, clique em **Mais informações** e depois em **Executar assim mesmo**. Esse aviso pode aparecer com programas novos que ainda não são conhecidos pelo Windows.
5. Siga o instalador (em português) até o fim.

<!-- Captura manual pendente (release v0.1.0): media/instalador-boas-vindas.png — primeira tela do instalador, em pt-BR. -->
<!-- Captura manual pendente (release v0.1.0): media/instalador-concluido.png — tela final do instalador. -->

A instalação é **só para o seu usuário do Windows**: não pede senha de administrador. Ao terminar, aparecem atalhos do **Master Of Notes** no menu Iniciar e na Área de Trabalho.

### Onde as coisas ficam

| O quê | Onde |
|---|---|
| O programa | `%LOCALAPPDATA%\Master Of Notes\` |
| Seus dados (biblioteca, cantores, pontuações, ajustes) | `%APPDATA%\com.masterofnotes.karaoke\` |
| Seus vídeos | Continuam na pasta onde você os guardou. O app não copia nem move nada. |

Para abrir uma dessas pastas, cole o caminho na barra de endereço do Explorador de Arquivos.

## Atualizar

Quando sair uma versão nova, o próprio app pode instalá-la. Veja [Atualizações](11-atualizacoes.md). Seus dados são mantidos.

## Desinstalar

1. Abra **Configurações do Windows → Aplicativos → Aplicativos instalados**.
2. Procure **Master Of Notes**, clique em **⋯** e em **Desinstalar**.

<!-- Captura manual pendente (release v0.1.0): media/desinstalar-windows.png — "Aplicativos instalados" com o Master Of Notes. -->

O desinstalador remove o programa e os atalhos. **Seus dados ficam guardados** em `%APPDATA%\com.masterofnotes.karaoke\`, para o caso de você instalar de novo. Se quiser apagar tudo, apague essa pasta depois de desinstalar. Isso remove seus rankings, cantores e ajustes de forma definitiva. Seus vídeos não são tocados.

---

[← Índice](README.md) · [Próximo: Primeira execução →](02-primeira-execucao.md)
