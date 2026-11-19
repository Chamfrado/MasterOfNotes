# Changelog

Formato: [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/); versionamento: SemVer.

## [Unreleased]

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
