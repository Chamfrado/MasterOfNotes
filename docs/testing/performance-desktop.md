# Desempenho do desktop (T-001-07)

Dono: Agente 1. A seção de áudio fica em [performance.md](performance.md) (Agente 2). As metas estão em [strategy.md](strategy.md) (seção Performance).

## Ambiente (medido em 2026-11-19)

- **Hardware:** Intel Core i5-11400H (6C/12T, 2,7 GHz), 31,8 GB de RAM, SSD NVMe 512 GB.
- **Sistema:** Windows 11 Home 10.0.26200, plano de energia "Equilibrado", WebView2 157.0.4341.60.
- **Código:** `agent-1/work` @ `8a5ff79`. Builds `tauri build --no-bundle` (release, LTO) e `tauri build --debug --no-bundle`, com um overlay que só troca o `identifier` para `com.masterofnotes.perftest`. Assim a medição usa um banco e um perfil WebView2 próprios e não toca nos dados do proprietário.
- **Biblioteca sintética:** 1.000 arquivos em 20 pastas, extensões alternadas (mp4/mkv/webm/mov/avi). 10% são esparsos de 50–200 MB (só cabeça e cauda gravadas, `fsutil sparse setflag`); o resto tem 4–256 KiB. Tamanho lógico de 12,8 GB, mas pouco espaço real em disco. O gerador é `library::tests::gen_library`.
- **Cache:** os arquivos tinham acabado de ser gerados, então o cache do SO estava quente. A primeira indexação com disco frio (pós-boot) **não** foi medida ⏳.

## Resultados

| Métrica | Meta | Release | Debug |
|---|---|---|---|
| Início do processo → 1ª resposta de IPC (`get_settings`) | — | mediana **1,17 s** (0,94–1,80 s; 1 outlier de 4,4 s; n=9) | mediana 2,23 s (1,64–2,55 s; n=5) |
| Início → navegação do WebView | — | mediana 0,94 s | — |
| Primeiro uso (perfil WebView2 e banco novos) → 1º IPC | — | 2,56 s (n=1) | — |
| Clique em "Biblioteca" → cards na tela (1.000 músicas) | — | 131–138 ms (n=3) | 129–159 ms (n=5) |
| **Catálogo visível** (1º IPC + Biblioteca) | < 2 s | **≈ 1,3 s** ✅ | ≈ 2,4 s |
| Indexação de 1.000 arquivos (`set_library_folder`, `ScanSummary.durationMs`) | < 30 s | **376–486 ms** ✅ | 386–486 ms (1 outlier de 15,9 s, ver ❓) |
| Re-varredura sem mudanças (`rescan_library`) | — | 78–96 ms | 98–123 ms |
| `list_songs` com 1.000 itens (IPC completo, medido no JS) | — | 27,8 ms | 43,5 ms |
| Memória com 1.000 itens, processo Rust (working set / privada) | — | 47 MB / 24 MB | 54 MB / 24 MB |
| Memória com 1.000 itens, Rust + 6 processos WebView2 (WS / privada) | — | 467–479 MB / 316–328 MB com a Biblioteca aberta; ≈ 390 / 257 MB na tela inicial | ≈ 475 / 320 MB |
| LiveTick → chegada no JS (frame → UI) | < 80 ms | **p50 52,6 ms, p95 67,3 ms**, máx 148 ms (n=603) ✅ | — |
| Intervalo entre LiveTicks | ~33 Hz | p50 29,2 ms, p95 39,6 ms, máx 125 ms | — |

Benchmark em processo (`cargo test -p master-of-notes perf_index -- --ignored --nocapture`, perfil dev): 1ª varredura 923 ms, re-varredura 144 ms, `list_songs` 16 ms.

## Como foi medido

- **Inicialização:** o app sobe com `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333 --remote-debugging-address=127.0.0.1`. Do lado de fora, anoto o instante do `Start-Process` em epoch ms. Depois, via CDP (Playwright `connectOverCDP`), leio `performance.timeOrigin` e o Resource Timing das chamadas a `http://ipc.localhost/<comando>`. O tempo de conexão do CDP não entra no número, porque os instantes vêm do próprio WebView.
  - A tela inicial chama só `get_settings`; `list_songs` só é chamado quando a Biblioteca abre. Por isso "catálogo visível" = 1º IPC + o tempo da Biblioteca (clique → primeiro card no DOM, medido em `requestAnimationFrame`).
  - Antes de cada início, espero sumirem os processos WebView2 do perfil de teste. Sem isso, uma instância nova se pendura no browser WebView2 que ficou da anterior e mede errado.
- **Indexação e re-varredura:** `set_library_folder` e `rescan_library` chamados pelo IPC real. O número é o `durationMs` que o próprio scanner devolve; o tempo total da chamada IPC também foi anotado (826 ms release na 1ª, incluindo a liberação do escopo do asset protocol para 1.000 arquivos).
- **Memória:** `WorkingSet64` e `PrivateMemorySize64` do processo Rust e dos `msedgewebview2.exe` cujo comando cita o perfil de teste.
- **LiveTick:**
  - importo a referência da música de teste (25 notas) e abro o Player real;
  - envolvo as entradas de `__TAURI_INTERNALS__.callbacks`, porque `transformCallback` não é gravável;
  - em cada tick com `feedback`, registro `video.currentTime·1000 − feedback.tMs`, com `latencyOffsetMs = 0`.
  - O `tMs` é o tempo de música do **centro** da janela de análise, então os ~21 ms de meia janela (2048 amostras a 48 kHz) entram no número: é a latência real frame → UI.
  - Também entram a âncora de sincronização (resync com tolerância de 40 ms) e a resolução do `currentTime`.
  - Microfone mudo, então os rótulos foram "Não detectado". Isso não muda o caminho medido.

Os scripts de CDP (`run.ps1`, `measure.mjs`, `ticks.mjs`) ficaram fora do repositório, no scratchpad da sessão do Agente 1. Se quiserem, viram ferramenta versionada.

## Observações ❓ e pendências ⏳

- ❓ **Outlier de 15,9 s** na primeira indexação do exe de debug recém-compilado. Repetida duas vezes do zero, deu 386 e 486 ms. A hipótese, não verificada, é a varredura do antivírus na primeira execução de um binário novo lendo 1.000 arquivos. Um início release de 4,4 s também ficou isolado; o resto ficou entre 0,94 e 1,80 s.
- ❓ A memória do WebView2 (~390–480 MB de working set somando 7 processos) domina. Grande parte é compartilhada com o runtime do Edge e conta mais de uma vez no WS. A memória privada somada (~320 MB) é a referência mais justa.
- A Biblioteca renderiza os 1.000 cards de uma vez (3.012 botões no DOM), em ~130 ms, sem virtualização. Ainda está ok com 1.000; considerar virtualizar acima de alguns milhares.
- ⏳ Primeira indexação com cache frio (pós-boot) e biblioteca em HD mecânico: não medidas.
- ⏳ CPU durante a performance: fica com o Agente 2 ([performance.md](performance.md)).
