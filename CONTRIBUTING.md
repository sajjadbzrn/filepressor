# Contributing to FilePressor

Thanks for your interest in improving FilePressor! This document explains how to
get the codebase running and how to propose changes.

## Getting Started

FilePressor is a [Tauri 2](https://v2.tauri.app/) app: a Vue 3 + TypeScript
frontend backed by a Rust core.

### Prerequisites

- **Rust** (stable) — https://rustup.rs
- **Node.js** (>= 18) and **Bun** — https://bun.sh
- **System webview** — on Linux install `libwebkit2gtk-4.1-dev`,
  `libappindicator3-dev`, `librsvg2-dev`, `patchelf`, `libgtk-3-dev`.

### Setup

```bash
bun install              # frontend dependencies
bun run tauri dev        # run the app with hot reload
```

The frontend alone (no Rust) can be started with `bun run dev`.

## Project Layout

- `src/` — Vue 3 frontend (`components/`, `composables/`, `lib/`)
- `src-tauri/` — Rust backend (`archives.rs`, `media.rs`, `tasks.rs`, `lib.rs`)
- `src-tauri/capabilities/` — Tauri permission capabilities

## Making Changes

1. Fork the repo and create a branch from `main`
   (`git checkout -b fix/my-change`).
2. Keep changes focused. Format Rust with `cargo fmt` and TypeScript with
   `bun run build` (runs `vue-tsc` type-checking).
3. Make sure both the frontend type-check and `bun run tauri build` succeed.
4. Open a pull request describing **what** changed and **why**.

### Commit Messages

Use clear, imperative commit messages, e.g. `fix: handle empty archive on
extract` or `feat: add AVIF output format`.

## Reporting Bugs & Ideas

- Use the **Bug Report** template for reproducible problems.
- Use the **Feature Request** template for suggestions.

## Code of Conduct

By participating, you agree to abide by our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Releasing

Maintainers cut releases by pushing a `v*` tag. The GitHub Actions release
workflow builds signed bundles for Windows and Linux and publishes a GitHub
Release with the auto-update metadata (`latest.json`). The signing private key
is provided through repository secrets — contributors do not need it for local
development.
