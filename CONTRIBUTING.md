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
git clone https://github.com/NaturalDevCR/TCP-Streamer.git
cd tcp-streamer
pnpm install
```

## Development

```bash
pnpm tauri dev
```

This starts the Vite dev server on port 1420 and opens the Tauri window.

### Available Scripts

| Command | Description |
|---------|-------------|
| `pnpm dev` | Start Vite dev server only (frontend) |
| `pnpm build` | Build frontend for production |
| `pnpm tauri dev` | Start full Tauri app in dev mode |
| `pnpm tauri build` | Build distributable binaries |
| `pnpm test` | Run Vitest (frontend) unit tests |
| `pnpm test:watch` | Run Vitest in watch mode |
| `pnpm lint` | Run ESLint |
| `pnpm lint:fix` | Auto-fix ESLint issues |
| `pnpm format` | Format with Prettier |
| `pnpm format:check` | Check Prettier formatting |
| `pnpm typecheck` | Run vue-tsc type checking |

### Backend (Rust)

```bash
cd src-tauri

# Run Rust tests
cargo test

# Lint with Clippy
cargo clippy

# Format
cargo fmt

# Check formatting
cargo fmt --check
```

## Before Submitting

Pre-commit hooks (lefthook) run automatically on `git commit`:

- Prettier formatting check
- ESLint
- `cargo fmt --check`
- `cargo clippy`

You can also run them manually:

```bash
pnpm lint && pnpm format:check && pnpm typecheck
cd src-tauri && cargo fmt --check && cargo clippy && cargo test
```

## Project Structure

```
src/                          — Vue 3 frontend (TypeScript + Tailwind CSS 4)
  components/
    AppShell.vue              — Application shell layout
    Header.vue                — Top navigation bar
    ToastNotification.vue     — Toast notification component
    icons.ts                  — Icon definitions
    sections/                 — Main views
      AudioSection.vue        — Input device, sample rate, buffer, latency
      ConnectionSection.vue   — Role, transport, mode, address, allowlist, PSK
      DashboardSection.vue    — Real-time streaming stats
      LogsSection.vue         — Log viewer with filter and search
      SettingsSection.vue     — Profiles, automation, language, about
    ui/                       — Reusable UI primitives (reka-ui based)
      Badge, Button, Card, CopyField, Dialog, Field, Input,
      SegmentedControl, Select, SettingLabel, Stat, StatusDot,
      Switch, Textarea, Tooltip
  composables/                — Tauri IPC bridge (useTauri)
  i18n/                       — Translation catalogs (en.json, es.json)
  stores/                     — Pinia state management (settings, stream, ui)
  types/                      — TypeScript event type definitions

src-tauri/src/                — Rust backend (Tauri v2)
  lib.rs                      — App builder, plugins, tray, window mgmt
  main.rs                     — Binary entry point
  audio/
    commands.rs               — Tauri IPC command handlers
    manager.rs                — AudioState: stream lifecycle
    engine/                   — Audio pipeline core
      mod.rs                  — engine::run() orchestrator
      buffer.rs               — Real adaptive latency-target controller
      capture.rs              — Real-time-safe capture (ring buffer producer)
      convert.rs              — Catmull-Rom stereo resampler
      decoder.rs              — s16le PCM → f32 decoder
      device.rs               — Device format + rate negotiation
      encoder.rs              — f32 → s16le PCM encoder
      latency.rs              — Latency profile definitions
      pacing.rs               — Starvation detection + backlog catch-up
      playback.rs             — Sink-mode output playback
      sink.rs                 — Sink output pipeline
    transport/                — Network layer
      mod.rs                  — Connection trait + TcpConnection
      tcp_client.rs           — TCP client (outbound)
      tcp_server.rs           — TCP server (inbound, HTTP detection)
      allowlist.rs            — IP/CIDR allowlist
      discovery.rs            — mDNS service advertisement/discovery
      dscp.rs                 — DSCP/QoS TOS mapping (tested)
      resolve.rs              — DNS/hostname resolution
      udp/
        mod.rs                — Native UDP transport root
        source.rs             — UDP audio source (packetization + send)
        sink.rs               — UDP audio sink (receive + de-jitter)
        packet.rs             — Packet format (sequence/timestamp framing)
        jitter.rs             — De-jitter buffer + loss concealment
        crypto.rs             — ChaCha20-Poly1305 AEAD + HKDF
        drift.rs              — Clock-drift compensation
        tests_loopback.rs     — UDP integration tests
    metrics.rs                — Quality scoring + TCP RTT via TCP_INFO
    stats.rs                  — Streaming statistics + event types
    constants.rs              — Retry delays, heartbeat intervals
    wav_helper.rs             — WAV header generation
    chunked.rs                — HTTP chunked transfer encoding
    error.rs                  — Error types
```

## Architecture

### Audio Pipeline

**Source (capture → send):**

```
Input Device → cpal capture (real-time-safe callback)
  → Lock-free Ring Buffer (f32, stereo)
  → Catmull-Rom Resampler (device rate → wire rate)
  → f32→s16le Encoder
  → Connection trait (TCP or UDP with encryption)
  → Network
```

**Sink (receive → play):**

```
Network → Connection trait (TCP or UDP with de-jitter)
  → s16le→f32 Decoder
  → Ring Buffer
  → Playback (cpal output with format negotiation)
```

### Key Design Points

- **Lock-free ring buffer** decouples the real-time audio callback from the network thread.
- **Connection trait** abstracts TCP and UDP behind a common `Write` + RTT interface, keeping the engine transport-agnostic.
- **Adaptive buffer controller** adjusts standing-latency target based on underrun/overrun signals.
- **Frame-aligned corrections** ensure buffer drops, catch-up, and drift adjustments never swap L/R channels.
- **No PTP required** for Native UDP — clock drift handled by occasional mini-chunk insert/drop.

### Testing

- **Frontend:** Vitest with happy-dom environment. Run: `pnpm test`
- **Backend:** `cargo test` in `src-tauri/`. Includes unit tests for encoder, decoder, resampler, device negotiation, latency profiles, buffer controller, metrics, DSCP mapping, packet framing, crypto, jitter buffer, drift, and UDP loopback integration tests.
