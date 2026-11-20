# Changelog

Formato: [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/); versionamento: SemVer.

## [Unreleased]

## [0.1.3] - 2026-11-20

### Adicionado
- **Gerar automaticamente** a referência melódica a partir do próprio vídeo, sem MIDI. O app separa a voz da música e transcreve as notas (leva cerca de 2,5 minutos para uma música de 4 minutos). Funciona em vídeos **com voz**; em karaokê só instrumental, o app avisa que não encontrou uma voz clara. A referência gerada nasce experimental: revise e valide.
  - Usa um **modelo de separação de voz de terceiros**, que você mesmo baixa (o app mostra a página do projeto e o link do arquivo, e não baixa nada sozinho). Só modelos conhecidos são aceitos (o arquivo é conferido). A licença dos pesos do modelo não é declarada pelos autores: o uso é decisão e responsabilidade de quem baixa.
  - Avisa quando a voz do vídeo está fraca e a referência pode ter ficado incompleta.
- **Alinhar automaticamente:** o app encontra sozinho o deslocamento entre a referência (MIDI, JSON, cantada ou gerada) e o vídeo e propõe o ajuste com o nível de confiança; você decide se aplica. Ele avisa quando não tem certeza (por exemplo, se o andamento da referência for diferente do vídeo).

### Componentes de terceiros
- O instalador passa a incluir o **ONNX Runtime** da Microsoft (licença MIT), usado só para a referência automática. O aviso de licença está em `THIRD-PARTY-NOTICES.txt`, na pasta do programa.

## [0.1.2] - 2026-11-19

### Adicionado
- **Gravar cantando:** sem o arquivo MIDI da música, alguém que conhece a melodia canta junto com o vídeo e o app cria a referência a partir das notas cantadas. O app guarda só as notas, nunca a voz. Depois é só revisar sobre o vídeo e validar.
- Medição de latência agora avisa quando a medida ficou imprecisa e recomenda repetir.

### Corrigido
- Atualizações: em vez de erro, o app mostra "Nenhuma versão nova pelo canal de atualizações automáticas" e um link para a página de versões.
- Esta é a primeira versão **normal** (não pré-release): a partir dela, a verificação automática de atualização funciona também para quem instalou a 0.1.1.
- O selo "Gravando notas" não cobre mais o vídeo.

## [0.1.1] - 2026-11-19 (pré-release)

Primeira versão pública. Pré-release: a pontuação foi calibrada e auditada com vozes sintéticas; a validação com microfone e voz reais ainda está em andamento.

### Adicionado
- Catálogo local: escolha uma pasta com vídeos (MP4, WEBM, MOV, MKV; AVI é reconhecido, mas não toca) e o app monta a biblioteca, com busca e atualização automática quando arquivos mudam.
- Player com contagem regressiva, tela cheia, volume, painel de afinação ao vivo (modos completo/compacto/oculto) e atalhos de teclado.
- Microfone: escolha do dispositivo, medidor ao vivo, calibração de ruído, teste de vazamento (obrigatório para valer ranking) e medição de latência automática ou manual.
- Referência melódica: importação de MIDI (com escolha da trilha) ou JSON, alinhamento visual sobre o vídeo e validação.
- Pontuação vocal (algoritmo v5): afinação 65%, ritmo 20%, sustentação 15%; vibrato e gestos expressivos não são penalizados; oitava acima/abaixo vale; volume não conta.
- Modos Solo, Festa (classificação por média), Duelo (só compara apresentações comparáveis) e Desafio, com participantes locais e retomada de partida.
- Ranking por música e dificuldade, histórico de apresentações (com "Versão anterior"), detalhes da música e recordes.
- Ajustes, Modo TV e navegação por setas (controle remoto).
- Atualizações automáticas assinadas, sempre com consentimento.
- Guia do usuário em `docs/user/`.

### Privacidade
- O áudio do microfone é analisado na hora e nunca é gravado.

## [0.1.0] - 2026-11-19 (não publicada)
- Build interna de validação do processo de release.
