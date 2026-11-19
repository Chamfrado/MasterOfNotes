# ADR-0001 — Stack tecnológica

- **Status:** Aceita · 2026-11-19 · Agente 0
- **Contexto:** app desktop offline, prioridade Windows, captura de áudio de baixa latência, DSP determinístico, reprodução de vídeo local, atualização assinada, produto comercializável.

## Decisão

| Área | Escolha | Licença |
|---|---|---|
| Shell desktop | Tauri 2 (WebView2 no Windows) | MIT/Apache-2.0 |
| Núcleo nativo | Rust 2024 (workspace Cargo) | — |
| UI | React 19 + TypeScript strict + Vite + Tailwind CSS v4 | MIT |
| Primitivos acessíveis | Radix UI (só os necessários: Dialog, Select, Slider, Toast) | MIT |
| Animação | Motion (`motion`) | MIT |
| Banco | SQLite via `rusqlite` (feature `bundled`) | MIT / domínio público |
| Áudio | `cpal` (WASAPI) | Apache-2.0 |
| Pitch | YIN implementado no crate (sem dependência) | — |
| MIDI | `midly` | Unlicense/MIT |
| Hash de conteúdo | `blake3` | CC0/Apache-2.0 |
| Watcher | `notify` (+ debounce) | CC0/Artistic |
| Atualização | `tauri-plugin-updater` + GitHub Releases | MIT/Apache-2.0 |
| Testes | Vitest, Testing Library, Playwright, `cargo test` | MIT |

## Alternativas avaliadas

- **Electron:** ecossistema maior, mas binário ~150 MB, Node no processo principal e DSP em JS/N-API. Rejeitado: Tauri entrega binário menor e Rust nativo para DSP.
- **App nativo (Qt/egui):** vídeo com codecs e UI rica custam mais; Qt tem implicações de licença (LGPL/comercial). Rejeitado.
- **Web Audio API no WebView para captura/pitch:** latência e agendamento menos previsíveis, testes determinísticos mais difíceis e acoplamento do DSP à UI. Rejeitado; Rust captura e analisa.
- **`tauri-plugin-sql` (SQL a partir do JS):** espalharia regras de persistência pela UI (viola Lei 04/05). Rejeitado; SQLite só via Rust.
- **aubio / crepe (ML) para pitch:** aubio é GPL (incompatível com distribuição proprietária); crepe exige modelo + runtime pesado. YIN é suficiente para voz monofônica e é determinístico. Reavaliar pYIN se fixtures mostrarem erros de oitava.
- **Biblioteca de componentes completa (MUI/Chakra):** estética "admin" e peso. Rejeitado; design system próprio sobre Radix.

## Consequências

- Codecs limitados ao que o WebView2 suporta (ver ADR-0003).
- Build Windows exige MSVC Build Tools + WebView2 (presentes no ambiente de referência).
- macOS/Linux: compiláveis, não priorizados; WebKitGTK/WKWebView têm suporte de codecs diferente.
