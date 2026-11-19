# MasterOfNotes

Karaokê desktop com pontuação vocal real: escolha uma pasta de vídeos, cante no microfone e seja avaliado contra a melodia de referência. Offline, sem conta.

> Status: em desenvolvimento (Fase 1). Veja [docs/roadmap.md](docs/roadmap.md).

## Desenvolvimento

Requisitos (Windows): Node 22+, pnpm 10, Rust stable (MSVC), Visual Studio Build Tools, WebView2.

```sh
pnpm install
pnpm dev                 # abre o app
cargo test --workspace   # testes Rust
```

Documentação: [AGENTS.md](AGENTS.md) · [leis](docs/project-laws.md) · [arquitetura](docs/architecture/overview.md) · [specs](specs/README.md)

## Conteúdo de terceiros
O app não inclui músicas, vídeos ou letras. O usuário é responsável pelos direitos do conteúdo que usa.
