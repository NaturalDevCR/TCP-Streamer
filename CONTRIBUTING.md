# Contributing to TCP Streamer

## Prerequisites

- [Node.js](https://nodejs.org/) 22+
- [pnpm](https://pnpm.io/) (v9+) — `npm i -g pnpm` or `corepack enable`
- [Rust](https://rustup.rs/) (stable toolchain)
- Platform-specific dependencies:
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev libssl-dev`
  - **Windows**: Microsoft Visual Studio C++ Build Tools

## Setup

```bash
git clone <repo-url>
cd tcp-streamer
pnpm install
```

## Development

```bash
pnpm tauri dev
```

This starts the Vite dev server on port 1420 and opens the Tauri window.

## Before Submitting

- Run linting: `pnpm lint` and `cargo clippy` (in `src-tauri/`)
- Run tests: `pnpm test` and `cargo test` (in `src-tauri/`)
- Check formatting: `pnpm format:check` and `cargo fmt --check` (in `src-tauri/`)
- Type check: `pnpm typecheck`

## Project Structure

```
src/              - Vue 3 frontend (TypeScript + Tailwind CSS)
  components/     - Reusable Vue components
  composables/    - Tauri bridge utilities
  stores/         - Pinia state management
  types/          - TypeScript type definitions
src-tauri/        - Rust backend (Tauri v2)
  src/audio/      - Audio capture and streaming engine
    engine/       - Device selection, capture, encoding, adaptive buffer
    transport/    - TCP client/server, DSCP, Connection trait
    metrics.rs    - Underrun/overrun counters, quality score, TCP RTT
```

## Architecture

The audio pipeline uses CPAL for capture and a lock-free ring buffer (`ringbuf`) to transfer audio from a real-time-safe capture callback to a network thread. The network thread sends PCM audio over TCP with optional HTTP chunked encoding (for `.wav` streaming in server mode), through a `Connection` trait that keeps the engine transport-agnostic.
