# TCP Streamer

> A feature-rich, cross-platform audio streaming desktop application built with Tauri. Capture, send, receive, and play back audio over TCP or encrypted Native UDP with sub-second latency.

![Version](https://img.shields.io/badge/version-2.3.0-blue.svg)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)
![License](https://img.shields.io/badge/license-MIT-green.svg)

---

## Screenshots

<img width="200" height="425" alt="image" src="https://github.com/user-attachments/assets/c490c1bd-3644-4812-978a-218c42d6b95c" />
<img width="200" height="425" alt="image" src="https://github.com/user-attachments/assets/b4eca9f7-5d82-4c64-a257-1e1402f468a0" />
<img width="200" height="425" alt="image" src="https://github.com/user-attachments/assets/4da8db28-77af-4a6b-b50d-e414295db4f7" />
<img width="200" height="425" alt="image" src="https://github.com/user-attachments/assets/e9b700f4-4a1f-4ac7-bdbd-22e8aa1e423c" />
<img width="200" height="425" alt="image" src="https://github.com/user-attachments/assets/8c54c4aa-dd60-4dd7-960f-52b0f3ef836c" />

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Transport Modes](#transport-modes)
- [Use Cases](#use-cases)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [System Requirements](#system-requirements)
- [Architecture](#architecture)
- [Development](#development)
- [Troubleshooting](#troubleshooting)

---

## Overview

**TCP Streamer** is a bidirectional audio streaming desktop application. It can operate as:

1. **Source** — Capture audio from an input device and send it over the network.
2. **Sink** — Receive an audio stream from the network and play it through a local output device.

Both roles support **TCP** (max compatibility, HTTP/WAV streaming) and **Native UDP** (low latency, encryption, mDNS discovery).

### How It Works

```
┌─────────────┐      ┌──────────────┐      ┌─────────────┐
│   Audio     │      │     TCP      │      │   Server    │
│   Input     │─────>│   Streamer   │─────>│  (Audio     │
│  (Device)   │      │   (Source)   │      │ Receiver)  │
└─────────────┘      └──────────────┘      └─────────────┘

┌─────────────┐      ┌──────────────┐      ┌─────────────┐
│  TCP/UDP    │      │     TCP      │      │   Audio     │
│  Source     │─────>│   Streamer   │─────>│   Output    │
│             │      │   (Sink)     │      │  (Device)   │
└─────────────┘      └──────────────┘      └─────────────┘
```

1. **Capture** — Reads audio from a selected input device with real-time-safe ring buffer callback.
2. **Resample & Encode** — Catmull-Rom resampler converts device rate to wire rate; f32 to s16le stereo encoding.
3. **Buffer** — Lock-free ring buffer absorbs network jitter with adaptive target-occupancy control.
4. **Send** — Transmits over TCP or encrypted Native UDP, with RTT measurement and quality scoring.
5. **Receive (Sink)** — De-jitter buffer with sequence tracking, clock-drift compensation, and loss concealment.
6. **Play** — Decodes received audio and plays through a selected output device.

---

## Features

### Roles & Transport

- **Source role** — Capture and send audio from any input device (microphone, loopback, virtual device).
- **Sink / Playback role** — Receive and play audio to any output device (speakers, virtual output).
- **TCP mode** — Universal compatibility with any TCP audio receiver; includes HTTP/WAV streaming.
- **Native UDP mode** — Sub-second latency with de-jitter buffer, loss concealment, and clock-drift compensation.
- **mDNS Discovery** — Native UDP sources automatically advertise on the LAN; sinks scan and pick them.
- **AEAD/PSK Encryption** — Optional per-packet ChaCha20-Poly1305 encryption for Native UDP with HKDF key derivation.
- **IPv4 & IPv6** — Dual-stack server bind with hostname resolution for client connections.
- **IP/CIDR Allowlist** — Restrict TCP server access to specific IP addresses or networks.

### Audio Quality

- **Fixed wire format** — Always stereo s16le at the configured output rate; internal resampling handles any capture device.
- **Mono, Stereo & Multichannel** — Mono duplicated to stereo; multichannel uses front left/right pair.
- **i32 capture support** — In addition to f32, i16, and u16 capture formats.
- **Catmull-Rom Resampler** — High-quality sample rate conversion between device and wire rates.
- **Real-time-safe capture** — No mutex or per-callback heap allocation; overrun counter.
- **CPAL buffer negotiation** — Hardware buffer size validated against device-supported range.
- **DSCP/QoS** — Traffic marking applied in both client and server modes (tested strategy-to-TOS mapping).

### Reliability

- **Real adaptive buffer** — Target-occupancy controller grows latency when glitches occur, shrinks when stable.
- **Honest metrics** — Real underrun/overrun counters, RTT via TCP_INFO, and quality scoring.
- **Starvation detection** — Pre-fill cushion prevents cold-start stutter; catch-up logic maintains target latency.
- **Frame-aligned corrections** — Buffer drops, skips, and drift corrections preserve L/R channel alignment.
- **Auto-reconnect** — Single connection policy with exponential backoff and jitter (source mode, TCP).

### User Interface

- **Enterprise dark theme** — Premium glassmorphism design with Vue 3, Tailwind CSS 4, and reka-ui components.
- **Bilingual (en/es)** — Full i18n with keyboard-accessible contextual tooltips on every configurable option.
- **Top navigation** — Dashboard, Connection, Audio, Logs, and Settings sections.
- **Dashboard** — Real-time streaming stats: uptime, bitrate, transferred, RTT, underruns, quality score.
- **Configuration profiles** — Save and switch between named presets (connection, audio, and automation settings).
- **Configurable latency profiles** — Ultra-low, Balanced, Robust, and Custom with tested buffer parameters.
- **System tray** — Background operation with show/quit menu and autostart support.

### Automation

- **Auto-start on boot** — Launch when the OS session starts.
- **Auto-stream** — Begin streaming immediately on startup using the selected profile.
- **Auto-reconnect** — Retry TCP connections automatically on failure.

---

## Transport Modes

### TCP Mode

Best for universal compatibility. Works with Snapcast, Mopidy, VLC, browsers, and any TCP audio receiver.

| Role   | Behavior |
|--------|----------|
| Source | Connects to a remote TCP server or listens for incoming TCP connections (client/server mode). |
| Sink   | Connects to a TCP audio source and plays received audio. |

In server mode, TCP Streamer auto-detects HTTP clients and serves a `.wav` stream for browser playback.

### Native UDP Mode

Best for low-latency streaming between two TCP Streamer instances or compatible receivers.

| Feature          | Description |
|------------------|-------------|
| De-jitter buffer | Sequence/timestamp framing with configurable jitter tolerance. |
| Loss concealment | Conceals missing packets to avoid audible gaps. |
| Clock drift      | Inserts/drops mini-chunks to track long-run clock differences (no PTP required). |
| Encryption       | Optional Chacha20-Poly1305 per-packet AEAD with PSK + HKDF-SHA256 key derivation. |
| mDNS             | Sources advertise via multicast DNS; sinks scan the LAN and auto-populate the source list. |

---

## Audio format & device compatibility

- **Fixed wire format:** Audio is always sent as `s16le`, stereo, at the configured output rate. For Snapcast, use the default 48 kHz setting with `sampleformat = 48000:16:2`:

  ```ini
  source = tcp://0.0.0.0:4953?name=TCP-Streamer&mode=client&sampleformat=48000:16:2
  ```

  TCP Streamer can run as either a TCP client or server.

- **Capture devices:** Mono, stereo, and multichannel devices are supported; multichannel input uses the front left/right pair. Devices may run at any sample rate because audio is resampled internally via Catmull-Rom interpolation. Supported capture formats are `f32`, `i16`, `u16`, and `i32`.
- **Output devices (sink mode):** f32, i16, u16, and i32 playback devices are supported with automatic format negotiation, including mono downmixing and sample-rate conversion.
- **Windows system audio:** Use the built-in WASAPI `[Loopback]` devices; no extra software is required. VB-Audio Cable also works as a regular input.
- **macOS system audio:** Install a virtual device such as BlackHole (`brew install blackhole-2ch`). BlackHole 2ch is recommended, while BlackHole 16ch also works by streaming its front pair. Create a Multi-Output Device in Audio MIDI Setup to keep hearing audio locally.
- **Linux system audio:** Select the PulseAudio/PipeWire monitor source for your output, for example with `pavucontrol` or `pactl list sources | grep -i monitor`. See [LINUX_GUIDE.md](LINUX_GUIDE.md) for more Linux setup details.

---

## Use Cases

### 1. Multi-Room Audio

Stream audio from your computer to an audio receiver such as Sonium, enabling synchronized playback across multiple rooms.

**Source (TCP Client):**

```bash
TCP Streamer (Source) → Audio Receiver (192.168.1.100:4953)
```

**Source (TCP Server)** — receiver connects to you:

```bash
TCP Streamer (Source :1704) ← Audio Receiver
```

### 2. Low-Latency Desktop-to-Desktop

Use Native UDP for sub-second latency between two computers running TCP Streamer.

```bash
Computer A (Source, UDP) ──mDNS──▶ Computer B (Sink, UDP)
```

The sink discovers the source via mDNS, or you can enter the address manually. Enable encryption with a shared PSK for secure audio.

### 3. Remote Audio Monitoring

Send audio from a microphone or monitoring device to a central server for recording or analysis.

### 4. Browser Playback

In TCP Server mode, open the HTTP URL (e.g., `http://LAN_IP:1704/stream.wav`) in any web browser to listen to the live stream.

---

## Installation

### Download Pre-built Binaries

Download the latest release for your platform:

**[→ Releases Page](https://github.com/NaturalDevCR/TCP-Streamer/releases)**

| Platform    | File Type             | Installation                         |
| ----------- | --------------------- | ------------------------------------ |
| **macOS**   | `.dmg`                | Open and drag to Applications        |
| **Windows** | `.msi` or `.exe`      | Run installer                        |
| **Linux**   | `.AppImage` or `.deb` | [See LINUX_GUIDE.md](LINUX_GUIDE.md) |

### Build from Source

#### Prerequisites

- [Node.js](https://nodejs.org/) (v22 or later)
- [pnpm](https://pnpm.io/) (v9 or later) — `npm i -g pnpm` or `corepack enable`
- [Rust](https://www.rust-lang.org/) (latest stable)
- **macOS**: Xcode Command Line Tools
- **Windows**: Microsoft Visual Studio C++ Build Tools
- **Linux**: `libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev libssl-dev`

#### Steps

```bash
# Clone repository
git clone https://github.com/NaturalDevCR/TCP-Streamer.git
cd TCP-Streamer

# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build for production
pnpm tauri build
```

---

## Quick Start

### 1. Choose Your Role

- **Source** — Capture audio from an input device (microphone, loopback, virtual device) and send it over the network.
- **Sink** — Receive audio from a network source and play it through a local output device (speakers).

### 2. Select Transport

- **TCP** — Maximum compatibility with any audio receiver (Snapcast, VLC, browsers). Choose Client or Server mode.
- **Native UDP** — Low latency with optional mDNS discovery and encryption. Ideal for TCP Streamer-to-TCP Streamer links.

### 3. Configure Connection

**Source (TCP Client):** Enter the target IP and port (e.g., `192.168.1.100:4953`).
**Source (TCP Server):** Set a listening port (e.g., `1704`) and optionally restrict access with the allowlist.
**Source (UDP):** Set the port; the source will be advertised via mDNS.
**Sink (UDP):** Scan discovered sources or enter a source address manually. Select an output device for playback.

### 4. Adjust Audio Settings

- **Output Sample Rate:** 48 kHz (recommended; must match receiver).
- **Capture Buffer:** 1024 frames (balanced).
- **Latency Profile:** Balanced for most networks; Ultra-low for wired LAN; Robust for unstable/WiFi networks.

### 5. Start Streaming

Click **Start**. The dashboard shows real-time stats — RTT, underruns, bitrate, and quality score.

---

## Configuration

### Latency Profiles

Controls coordinated ring buffer and chunk size presets. The adaptive controller adjusts the standing-latency target within the profile's min/max band based on observed glitches.

| Profile     | Ring (ms) | Adaptive Band (ms) | Chunk  | Use Case |
|-------------|-----------|---------------------|--------|----------|
| **Ultra-low** | 2000 | 100 – 500 | 256 | Wired LAN, minimal latency |
| **Balanced**  | 4000 | 200 – 1500 | 512 | Default; good trade-off for most networks |
| **Robust**    | 8000 | 500 – 3000 | 1024 | Unstable/WiFi networks, high jitter tolerance |
| **Custom**    | Manual | Manual (Min/Max Buffer sliders) | Manual | Full control over all parameters |

Loopback (WASAPI) capture uses higher floors in each profile for extra stability.

### Connection Settings

- **Transport:** TCP (universal) or Native UDP (low-latency + discovery + encryption).
- **Mode (TCP):** Client connects to a remote receiver; Server listens for incoming connections.
- **Target Address:** Remote `host:port` for TCP client or Native UDP sink.
- **Allowlist:** Comma-separated IP/CIDR entries to restrict server access (empty = allow all).
- **Encryption Key (PSK):** Shared secret for Native UDP encryption. Leave empty for unencrypted transport.
- **Output Device (Sink):** Speakers or virtual output for received audio playback.

### Audio Settings

- **Input Device:** Microphone, monitor, loopback, or virtual capture device.
- **Output Sample Rate:** Wire rate sent to the receiver; must match the receiver's expected format.
- **Capture Buffer (frames):** CPAL hardware buffer size. Smaller = lower latency; larger = fewer glitches.
- **Stream Format:** PCM (raw audio) or WAV (with header for browser/VLC playback in TCP Server mode).
- **Adaptive Buffer:** Enables automatic buffer resizing within configured min/max limits.
- **Loopback (Windows):** Captures system audio directly via WASAPI without virtual cables.

### Settings

- **Configuration Profiles:** Save and switch between named groups of connection, audio, and automation settings.
- **Automation:** Auto-start on boot, auto-stream on launch, auto-reconnect on TCP connection loss.
- **Language:** Switch between English and Spanish for all UI labels and tooltips.

---

## Architecture

### Technology Stack

| Layer     | Technology |
|-----------|------------|
| Frontend  | Vue 3 (Composition API), Pinia, Tailwind CSS 4, reka-ui, Vite 7 |
| i18n      | vue-i18n (English + Spanish) |
| Backend   | Rust (Tauri v2) |
| Audio     | cpal (cross-platform audio), ringbuf (lock-free ring buffer) |
| Network   | socket2, mdns-sd, chacha20poly1305, hkdf, sha2 |
| Storage   | tauri-plugin-store (settings persistence) |
| Testing   | Vitest (frontend), cargo test (backend) |

### Project Structure

```
src/                     — Vue 3 frontend (TypeScript + Tailwind CSS)
  components/
    sections/            — Audio, Connection, Dashboard, Logs, Settings
    ui/                  — Badge, Button, Card, Dialog, Field, Input, Select,
                           SegmentedControl, SettingLabel, Switch, Tooltip, etc.
  composables/           — Tauri IPC bridge (useTauri)
  i18n/                  — English and Spanish translation catalogs
  stores/                — Pinia stores: settings, stream, ui
  types/                 — TypeScript event type definitions

src-tauri/src/           — Rust backend (Tauri v2)
  audio/
    commands.rs          — Tauri IPC command handlers
    manager.rs           — AudioState: stream lifecycle management
    engine/
      mod.rs             — Core orchestrator (engine::run): 1058 lines
      buffer.rs          — Real adaptive latency-target controller
      capture.rs         — Real-time-safe audio capture (ring buffer producer)
      device.rs          — Device format + rate negotiation
      convert.rs         — Catmull-Rom stereo resampler
      encoder.rs         — f32 → s16le PCM encoder
      decoder.rs         — s16le PCM → f32 decoder
      latency.rs         — Latency profiles (Ultra-low, Balanced, Robust, Custom)
      pacing.rs          — Starvation detection + backlog catch-up
      playback.rs        — Sink-mode output playback
      sink.rs            — Sink output pipeline
    transport/
      mod.rs             — Connection trait + TcpConnection
      tcp_client.rs      — TCP client (outbound connections)
      tcp_server.rs      — TCP server (inbound connections, HTTP detection)
      dscp.rs            — DSCP/QoS TOS mapping (tested)
      resolve.rs         — DNS/hostname resolution
      allowlist.rs       — IP/CIDR allowlist for server mode
      discovery.rs       — mDNS service advertisement/discovery
      udp/
        mod.rs           — Native UDP transport root
        source.rs        — UDP audio source (packetization + send)
        sink.rs          — UDP audio sink (receive + de-jitter)
        packet.rs        — UDP packet format (sequence/timestamp framing)
        jitter.rs        — De-jitter buffer with loss concealment
        crypto.rs        — ChaCha20-Poly1305 AEAD encryption + HKDF
        drift.rs         — Clock-drift compensation
    metrics.rs           — Quality scoring + TCP RTT measurement
    stats.rs             — Streaming statistics and event types
    constants.rs         — Retry delays, heartbeat intervals
    wav_helper.rs        — WAV header generation
    chunked.rs           — HTTP chunked transfer encoding
    error.rs             — Error types
```

### Audio Pipeline

**Source (TCP/UDP):**
```
Input Device → cpal capture (real-time-safe) → Ring Buffer (f32, stereo)
  → Catmull-Rom Resampler → f32→s16le Encoder
  → [TCP: Connection trait write] / [UDP: crypto + packetization]
  → Network
```

**Sink (TCP/UDP):**
```
Network → [TCP: Connection trait read] / [UDP: de-jitter + crypto]
  → s16le→f32 Decoder → Ring Buffer
  → Playback (cpal output device with format negotiation)
```

### Key Design Decisions

- **Lock-free ring buffer** completely decouples the real-time audio callback from the network thread.
- **Connection trait** abstracts TCP/UDP behind a common `Write` + RTT interface, keeping the engine transport-agnostic.
- **Adaptive buffer controller** adjusts the standing-latency target based on underrun/overrun signals — grows when glitching, shrinks when stable.
- **Frame-aligned corrections** ensure buffer drops, catch-up, and drift adjustments never swap left/right channels.
- **No PTP required** for Native UDP — clock drift is handled by occasional mini-chunk insert/drop on the sink side.

---

## Troubleshooting

### WASAPI Loopback Stuttering (Windows)

**Solutions:**
- Enable **Adaptive Buffer**.
- Ensure speakers/headphones are connected and active (WASAPI requires an active output).
- Increase the latency profile to **Robust** or use manual buffer settings.

### Connection Issues

**Server Mode:**
- Ensure the port (e.g., 1704) is allowed through your firewall.
- Verify the client (audio receiver/browser) can reach your IP.
- Check the allowlist does not block the client's IP.

**Client Mode:**
- Verify the target IP and port are correct.
- Check if the target server is online.
- Ensure DSCP/QoS settings match your network requirements.

**Native UDP:**
- Both endpoints must use the same PSK if encryption is enabled.
- mDNS requires multicast to be allowed on the local network segment.
- For manual addresses, verify `host:port` is reachable from the sink machine.

### Permission Errors (macOS)

**Problem:** "App is damaged and can't be opened"

**Solution:**
```bash
xattr -cr /Applications/TCP\ Streamer.app
```

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

## Support

- **Issues**: [GitHub Issues](https://github.com/NaturalDevCR/TCP-Streamer/issues)
- **Discussions**: [GitHub Discussions](https://github.com/NaturalDevCR/TCP-Streamer/discussions)

## Support the Project

If you find TCP Streamer useful and would like to support its development, consider making a donation:

[![Donate with PayPal](https://img.shields.io/badge/Donate-PayPal-blue.svg)](https://paypal.me/NaturalCloud)

---

**Made with love for the audio streaming community**
