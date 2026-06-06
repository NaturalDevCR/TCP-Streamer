# Contributing to TCP Streamer

## Prerequisites

- [Node.js](https://nodejs.org/) 22+
- [Rust](https://rustup.rs/) (stable toolchain)
- Platform-specific dependencies:
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev libssl-dev`
  - **Windows**: Microsoft Visual Studio C++ Build Tools

## Setup

```bash
git clone <repo-url>
cd tcp-streamer
npm install
```

## Development

```bash
npm run tauri dev
```

This starts the Vite dev server on port 1420 and opens the Tauri window.

## Before Submitting

- Run linting: `npm run lint` and `cargo clippy` (in `src-tauri/`)
- Run tests: `npm test` and `cargo test` (in `src-tauri/`)
- Check formatting: `npm run format:check` and `cargo fmt --check` (in `src-tauri/`)
- Type check: `npm run typecheck`

## Project Structure

```
src/              - Vue 3 frontend (TypeScript + Tailwind CSS)
  components/     - Reusable Vue components
  composables/    - Tauri bridge utilities
  stores/         - Pinia state management
  types/          - TypeScript type definitions
src-tauri/        - Rust backend (Tauri v2)
  src/audio/      - Audio capture and streaming engine
```

## Architecture

The audio pipeline uses CPAL for capture and a lock-free ring buffer (`ringbuf`) to transfer audio from the capture thread to a network thread. The network thread sends PCM audio over TCP with optional HTTP chunked encoding (for `.wav` streaming in server mode).
