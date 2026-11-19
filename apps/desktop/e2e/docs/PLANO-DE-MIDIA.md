# Plano de mídia do guia do usuário (SPEC-011, T-011-01)

Fonte da verdade para o script de captura (T-011-02). Cada linha vira um passo do script; o ID é o nome do arquivo em `docs/user/media/`.

## Regras

- **Origem:** Playwright contra o Vite (porta 1450 no script; `E2E_PORT=1455` se rodar via `playwright test`), com `e2e/tauri-mock.ts` + `e2e/fixtures.ts` + `e2e/fake-backend.ts` **importados, nunca alterados** (mudança → Agente 4).
- **Dados:** títulos, artistas e cantores inventados (Lei 19). Vídeo no player/referência = música de teste gerada por `e2e/tools/test-song.ts` (autoria própria). Pontuação e feedback ao vivo injetados pelo script via `__TAURI_INTERNALS__.runCallback` no canal do `start_performance` / `start_mic_monitor` → **legenda "valores ilustrativos"** em toda imagem com número (Lei 03).
- **Formato:** viewport 1280×800, tema padrão, `reducedMotion: "off"` nos GIFs (mostra as transições reais) e `"on"` nos PNG (estável). 1 exemplo em Modo TV.
- **Orçamento (R4):** PNG ~150 KB · GIF ≤ 2 MB (meta 1 MB) · vídeo ≤ 5 MB (meta 3 MB) · total ≤ 40 MB (meta ≤ 25 MB). O script falha se algum arquivo passar do limite.
- **Acessibilidade (R7):** cada mídia já nasce com o texto alternativo definido aqui; o passo também está escrito no texto da página.
- **Vídeo:** só link opcional abaixo do GIF (o GitHub não toca `<video>` do repositório).

Tipos: **P** = screenshot PNG · **G** = GIF (≤ 15 s) · **V** = vídeo WebM.

## Por página

### 01-instalacao.md
| ID | Tipo | Conteúdo | Origem |
|---|---|---|---|
| — | — | **Sem captura automática.** O instalador NSIS e o "Aplicativos instalados" do Windows não rodam no Playwright. Proposta: texto passo a passo agora; 2–3 screenshots manuais do instalador real (pt-BR) depois, quando o disco permitir um `pnpm build` (ou do release oficial). | manual ⏳ |

### 02-primeira-execucao.md
| ID | Tipo | Conteúdo | Estado do mock |
|---|---|---|---|
| inicio-primeira-vez | P | Início com a pergunta de atualizações e os dois passos pendentes | `settings(null)`, `updateConsentAsked:false` |
| inicio-pronto | P | Início com Biblioteca e Microfone marcados ✓ | pasta + `noiseFloorDbfs` definidos, consentimento respondido |
| menu-principal | P (recorte) | Barra de navegação com os 6 itens | qualquer |

### 03-biblioteca.md
| ID | Tipo | Conteúdo | Estado do mock |
|---|---|---|---|
| biblioteca-vazia | P | "Comece escolhendo sua pasta de músicas" | `libraryFolder:null`, `list_songs: []` |
| biblioteca-escolher-pasta | G | Clicar **Escolher pasta** → "Procurando vídeos…" → toast "N músicas encontradas" → grade aparece | `plugin:dialog|open` devolve `C:/Karaoke`; `set_library_folder` com atraso ~1 s |
| biblioteca-grade | P | Grade com selos de referência variados + 1 **Formato incompatível** (AVI) | 6–8 músicas inventadas, mix de status |
| biblioteca-lista | P | Modo **Lista** | idem |
| biblioteca-busca | G | Digitar "cancao" acha "Canção da Manhã" (sem acento) | idem |
| biblioteca-editar | P | Diálogo **Editar música** | idem |
| detalhes-musica | P | Tela da música: Informações, Reprodução, Referência melódica, Recordes, Suas apresentações (com **Treino** e **Versão anterior**) | `get_leaderboard`, `list_participants`, `list_performances` com dados de exemplo |
| detalhes-incompativel | P (recorte) | Bloco **Reprodução** com a mensagem de codec | música `playable:"no"` |

### 04-microfone.md
| ID | Tipo | Conteúdo | Estado do mock |
|---|---|---|---|
| microfone-tela | P | Tela inteira: dispositivo, Nível ao vivo, calibração, latência, volume, teste de vazamento | 2 dispositivos (fixture) |
| microfone-voz-detectada | G | Medidor subindo, selo **Voz detectada** e nota aparecendo | frames sintéticos no canal do monitor |
| microfone-calibrar | G | **Calibrar** → "Medindo… silêncio, por favor" → "Ambiente bom para cantar." | `calibrate_noise` com atraso de 3 s |
| microfone-calibrar-problemas | P (recorte) | Resultado com avisos (barulhento / saturando) | `issues:["noisy","clipping"]` |
| microfone-latencia-auto | P (recorte) | "Latência medida: … Ajuste salvo." | `finish_performance` com `latency` de exemplo |
| microfone-vazamento | P (recorte) | Teste de vazamento **Aprovado** e outro **Reprovado** com a orientação | `leakCheck` passed / failed |

### 05-referencia-melodica.md
| ID | Tipo | Conteúdo | Estado do mock |
|---|---|---|---|
| referencia-vazia | P | "Sem referência" + **Importar referência** | `get_reference: null` |
| referencia-trilha | P | Diálogo **Qual trilha é a voz?** com 3 trilhas | `list_midi_tracks` com nomes inventados |
| referencia-alinhar | G | Vídeo de teste tocando com as notas por cima; ajuste com **+100 ms** até alinhar; **Salvar offset** | música de teste + sua referência JSON |
| referencia-validar | P | Após **Validar alinhamento**: selo **Referência validada** | `set_reference_status` → validated |

### 06-cantar.md
| ID | Tipo | Conteúdo | Estado do mock |
|---|---|---|---|
| cantar-comecar | G | **Cantar** → tela "Começar" → contagem 3-2-1 → vídeo começa | música de teste, competitivo |
| cantar-painel-completo | P | Player com painel completo (nota, afinação) e pontuação | ticks sintéticos (ilustrativo) |
| cantar-controles | P | Barra inferior: Pausar, posição, Volume, Painel (H), Tela cheia (F), Sair (Esc) | pausado |
| cantar-painel-modos | G | Tecla H alterna completo → compacto → oculto | ticks sintéticos |
| cantar-pular-trecho | P | Diálogo **Pular trecho?** | seta → durante partida valendo |
| cantar-treino-vazamento | P | "Para valer ranking, faça antes o teste de vazamento…" + **Começar como treino** | referência validada, `leakCheck:null` |
| cantar-aviso-vazamento | P (recorte) | "O microfone está captando a música — use fones de ouvido." | ticks com padrão de vazamento |
| resultado | P | Resultado com nota, 4 métricas e **Novo recorde pessoal!** | `outcome` da fixture (ilustrativo) |
| resultado-sem-referencia | P | Resultado sem referência com métricas exploratórias | `result:null`, `exploratory` |
| cantar-fluxo-completo | V | Do Começar ao Resultado (~40 s, música de teste) | link opcional |

### 07-pontuacao.md
| ID | Tipo | Conteúdo | Origem |
|---|---|---|---|
| — | — | Sem captura de tela. Proposta: 1 diagrama SVG simples (afinação 65% · ritmo 20% · sustentação 15%; faixas de tolerância por dificuldade), escrito à mão no repositório. ❓ confirmar se a fórmula a citar é a do algoritmo v2 em vigor. | manual |

### 08-modos-de-jogo.md
| ID | Tipo | Conteúdo | Estado do mock |
|---|---|---|---|
| jogar-modos | P | Tela **Jogar** com os 4 modos + "Continuar de onde parou" | `fake-backend` com 1 sessão ativa |
| participantes | P | Tela **Participantes** com cantores inventados | `fake-backend` |
| festa-criar | G | Criar festa: nome, escolher cantores, músicas, ordem → **Criar festa** | `fake-backend` |
| festa-rodada | P | Partida em andamento: rodada atual, ranking parcial, lista de rodadas | `fake-backend`, notas ilustrativas |
| festa-vencedor | P | Festa encerrada com **Vencedor da festa** | `fake-backend` |
| duelo | P | Duelo com VS e vencedor | `fake-backend` |
| desafio | P | Desafio com meta, média e progresso | `fake-backend` |
| festa-completa | V | Fluxo da festa de ponta a ponta (base: `party.e2e.ts`) | link opcional |

### 09-ranking-e-historico.md
| ID | Tipo | Conteúdo | Estado do mock |
|---|---|---|---|
| ranking | P | Ranking com seletor de música e dificuldade | `get_leaderboard` (fixture) |
| historico | P | **Histórico de partidas** com Em andamento / Encerrada / Abandonada | `fake-backend` |
| (reuso) detalhes-musica | — | Recordes e Suas apresentações | página 03 |

### 10-ajustes-e-modo-tv.md
| ID | Tipo | Conteúdo | Estado do mock |
|---|---|---|---|
| configuracoes | P | Tela **Configurações** inteira | fixture |
| modo-tv | P | Início (ou Biblioteca) com **Modo TV** ligado | `tvMode:true` |
| navegacao-setas | G | Foco andando pela grade da Biblioteca só com setas + Enter | grade da página 03 |

### 11-atualizacoes.md
| ID | Tipo | Conteúdo | Estado do mock |
|---|---|---|---|
| atualizacao-disponivel | P | "Versão X disponível" + notas + **Baixar atualização** | `plugin:updater|check` com versão fictícia ❓ |
| atualizacao-instalar | P | Diálogo **Instalar agora?** | download simulado (só no script de captura) |
| atualizacao-em-dia | P (recorte) | "Você já está na versão mais recente." | `check` → null |

> ❓ O mock atual não cobre os comandos do plugin updater. Se não der para responder pelo `MockSpec` (sem alterar arquivos do Agente 4), estas imagens ficam para depois ou eu peço o suporte ao Agente 4.

### 12-privacidade.md, 13-problemas-e-faq.md, 14-glossario.md
Só texto; reusam recortes das páginas 03, 04 e 06 (ex.: `detalhes-incompativel`, `microfone-calibrar-problemas`).

## Totais estimados
| Tipo | Qtd | Estimativa |
|---|---|---|
| PNG | ~33 | ~5 MB |
| GIF | ~9 | ~9 MB (meta 1 MB cada) |
| Vídeo | 2 | ~6 MB |
| **Total** | | **~20 MB** (limite 40 MB) |

## GIF sem ffmpeg — proposta
1. O script tira screenshots em sequência (≈ 8 fps) com `page.screenshot()` durante o fluxo.
2. Codifica com **`gifenc`** (MIT, sem dependências, ~10 KB; quantização + paleta + LZW em JS puro), como **devDependency** do `apps/desktop` (pedido ao Agente 3).
3. Para caber em ≤ 2 MB: GIF reduzido a 960×600, paleta de 128 cores, e quadros parados repetidos viram atraso maior (não frame novo).

Alternativas descartadas: APNG/WebP montado à mão (APNG sem perdas fica 5–10× maior que o orçamento; WebP animado exige encoder nativo); codificar GIF no próprio script sem biblioteca (LZW + quantização ≈ 150 linhas para manter).

Vídeo: `recordVideo` nativo do Playwright (WebM VP8), sem dependência.
