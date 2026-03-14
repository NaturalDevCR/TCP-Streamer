# TCP Streamer

> A lightweight, cross-platform audio streaming application built with Tauri. Stream system audio over TCP with minimal latency and robust architecture.

![Version](https://img.shields.io/badge/version-2.0.0-blue.svg)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)
![License](https://img.shields.io/badge/license-MIT-green.svg)

---

## Screenshots

<img width="200" height="425" alt="image" src="https://github.com/user-attachments/assets/c490c1bd-3644-4812-978a-218c42d6b95c" />
<img width="200" height="425" alt="image" src="https://github.com/user-attachments/assets/b4eca9f7-5d82-4c64-a257-1e1402f468a0" />
<img width="200" height="425" alt="image" src="https://github.com/user-attachments/assets/4da8db28-77af-4a6b-b50d-e414295db4f7" />
<img width="200" height="425" alt="image" src="https://github.com/user-attachments/assets/e9b700f4-4a1f-4ac7-bdbd-22e8aa1e423c" />
<img width="200" height="425" alt="image" src="https://github.com/user-attachments/assets/8c54c4aa-dd60-4dd7-960f-52b0f3ef836c" />

## 📖 Table of Contents

- [Overview](#overview)
- [Features](#features)
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

**TCP Streamer** is a desktop application designed to capture and stream audio from your computer. It can operate in two modes:

1.  **Client Mode**: Connects to an existing Snapcast server (or any TCP server) and pushes audio to it.
2.  **Server Mode**: Acts as a TCP server, listening for incoming connections from Snapcast servers or media players.

It's perfect for integrating with multi-room audio systems like Snapcast, creating custom audio pipelines, or building distributed audio setups.

### How It Works

```
┌─────────────┐      ┌──────────────┐      ┌─────────────┐
│   Audio     │      │     TCP      │      │   Server    │
│   Input     │─────▶│   Streamer   │─────▶│ (Snapcast,  │
│  (Device)   │      │              │      │  Custom)    │
└─────────────┘      └──────────────┘      └─────────────┘
```

1.  **Capture**: Reads audio from a selected input device at a configurable sample rate (44.1kHz / 48kHz).
2.  **Buffer**: Pushes audio into a lock-free **Ring Buffer** to absorb network jitter.
3.  **Process**: Converts audio to raw PCM (16-bit signed, little-endian, stereo).
4.  **Stream**: Sends data over TCP.
    - **Client Mode**: Initiates connection to a target IP:Port.
    - **Server Mode**: Binds to a local Port and waits for connections.
    - **HTTP Stream**: In Server Mode, also provides a `.wav` stream for browser playback.

---

## Features

### Core Functionality

- ✅ **Modern Tabbed UI** - Clean, intuitive interface powered by Vue 3 and Tailwind CSS with a premium dark glassmorphism design.
- ✅ **Dual Operation Modes** - Run as a **Client** (push to server) or **Server** (listen for connections).
- ✅ **Raw PCM Audio** - High-quality, uncompressed audio streaming (16-bit, stereo preserving full L/R separation).
- ✅ **WAV Streaming** - Browser-compatible HTTP stream in Server Mode.
- ✅ **Connection Reliability** - Auto-reconnection logic and deep sleep mode.
- ✅ **Robust Audio Engine** - Native F32 internal architecture with safe PCM conversion.
- ✅ **Adaptive Buffer Sizing** - Automatically adjusts buffer based on network jitter.
- ✅ **Multi-Profile Support** - Save and switch between configurations for different environments.
- ✅ **System Tray Integration** - Runs in background, accessible from tray.
- ✅ **Windows Native Loopback** - Capture system audio without virtual cables (WASAPI).

### Audio Configuration

- 📊 **Sample Rates**: 44.1 kHz or 48 kHz.
- 🔧 **Buffer Sizes**: 256, 512, 1024, or 2048 samples.
- 🎤 **Input Devices**: Scans all host APIs (WASAPI, MME, CoreAudio) to find all devices.
- 🎚️ **Visual Volume Indicator**: Real-time RMS meter.

### Automation

- 🚀 **Auto-start on Boot** - Launch automatically when system starts.
- 🔄 **Auto-stream** - Begin streaming immediately on startup.
- 🔒 **Auto-reconnect** - Retry connection on failure.

---

## Use Cases

### 1. **Multi-Room Audio with Snapcast**

Stream audio from your computer to a Snapcast server, enabling synchronized playback across multiple rooms.

**Client Mode**:

```bash
TCP Streamer (Client) → Snapserver (192.168.1.100:4953)
```

**Server Mode** (Snapserver connects to you):

```bash
TCP Streamer (Server :1704) ← Snapserver
```

### 2. **Remote Audio Monitoring**

Send audio from a microphone or monitoring device to a central server for recording or analysis.

### 3. **Browser Playback**

In **Server Mode**, open the provided HTTP URL (e.g., `http://LAN_IP:1704/stream.wav`) in any web browser to listen to the live stream.

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

- [Node.js](https://nodejs.org/) (v18 or later)
- [Rust](https://www.rust-lang.org/) (latest stable)
- **macOS**: Xcode Command Line Tools
- **Windows**: Microsoft Visual Studio C++ Build Tools

#### Steps

```bash
# Clone repository
git clone https://github.com/NaturalDevCR/TCP-Streamer.git
cd TCP-Streamer

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

---

## Quick Start

### 1. **Select Input Device**

Choose the audio source you want to stream (microphone, virtual audio device, etc.).

> **Windows Loopback**: Check the **"Enable Loopback (Windows)"** box to capture system audio directly.
> **macOS Loopback**: Use software like [BlackHole](https://github.com/ExistentialAudio/BlackHole) or [Loopback](https://rogueamoeba.com/loopback/) to capture system audio.

### 2. **Select Mode**

- **Client Mode**: Select if you have a Snapserver running elsewhere. Enter its **Target IP** and **Port**.
- **Server Mode**: Select if you want Snapserver or a browser to connect _to you_. Set a local **Port** (e.g., 1704).

### 3. **Adjust Settings**

- **Sample Rate**: 48 kHz (recommended).
- **Buffer Size**: 1024 (balanced).

### 4. **Start Streaming**

Click **Start Streaming**.

- **Client Mode**: Attempting to connect to target...
- **Server Mode**: Listening on port... (Use the "Server Running" card to copy connection URLs).

---

## Configuration

### Server Mode Details

When running in **Server Mode**, TCP Streamer provides:

1.  **Snapcast TCP Stream**:
    - URL: `tcp://<YOUR_IP>:<PORT>`
    - Add to `snapserver.conf`:
      ```ini
      [stream]
      source = tcp://<YOUR_IP>:<PORT>?name=TCPStreamer&mode=client
      ```

2.  **Browser HTTP Stream**:
    - URL: `http://<YOUR_IP>:<PORT>/stream.wav`
    - Play directly in Chrome, Firefox, Safari, or VLC.

### Adaptive Buffer

Automatically resizes the ring buffer based on network conditions to prevent audio dropouts.

- **Enable**: Toggles the adaptive logic.
- **Min/Max Buffer**: Sets the valid range for automatic adjustment.

### Network Presets

Located in the **Advanced** tab:

| Preset | Description | Settings Applied |
| ~ | ~ | ~ |
| **Ethernet** | For stable wired connections | Ring Buffer: 2s, Chunk: 512 |
| **WiFi** | For standard wireless | Ring Buffer: 4s, Chunk: 1024 |
| **WiFi (Poor)** | For unstable/far connections | Ring Buffer: 8s, Chunk: 2048 |

---

## Architecture

### Technology Stack

- **Frontend**: Vue 3 (Composition API), Pinia (State Management), Tailwind CSS 4, Vite
- **Backend**: Rust (Tauri v2)
- **Audio**: cpal (cross-platform audio library)
- **Storage**: tauri-plugin-store (settings persistence)

### Audio Pipeline

```rust
Input Device → cpal → Producer → Ring Buffer → Consumer (Thread) → TCP/HTTP Stream
```

- **Format**: Raw PCM, 16-bit signed integers, little-endian
- **Channels**: 2 (stereo)
- **Buffering**: Lock-free Ring Buffer

---

## Troubleshooting

### WASAPI Loopback Stuttering (Windows)

**Solutions**:

- Enable **Adaptive Buffer**.
- Ensure speakers/headphones are connected and active (WASAPI requires an active output).
- Expect higher latency on WiFi (4-8 seconds).

### Connection Issues

**Server Mode**:

- Ensure the port (e.g., 1704) is allowed through your firewall.
- Ensure the client (Snapserver/Browser) can reach your IP.

**Client Mode**:

- Verify the target IP and Port are correct.
- Check if the target server is online.

### Permission Errors (macOS)

**Problem**: "App is damaged and can't be opened"

**Solution**:

```bash
xattr -cr /Applications/TCP\ Streamer.app
```

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

- **Issues**: [GitHub Issues](https://github.com/NaturalDevCR/TCP-Streamer/issues)
- **Discussions**: [GitHub Discussions](https://github.com/NaturalDevCR/TCP-Streamer/discussions)

## Support the Project

If you find TCP Streamer useful and would like to support its development, consider making a donation:

[![Donate with PayPal](https://img.shields.io/badge/Donate-PayPal-blue.svg)](https://paypal.me/NaturalCloud)

---

**Made with ❤️ for the audio streaming community**
