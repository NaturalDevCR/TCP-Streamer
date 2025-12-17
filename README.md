# TCP Streamer

> A lightweight, cross-platform audio streaming application built with Tauri. Stream system audio over TCP with minimal latency and robust architecture.

![Version](https://img.shields.io/badge/version-1.5.6-blue.svg)
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

**TCP Streamer** is a desktop application designed to capture and stream audio from your computer to any TCP server. It's perfect for integrating with multi-room audio systems like Snapcast, creating custom audio pipelines, or building distributed audio setups.

### What It Does

- **Captures** audio from any input device (microphone, system audio via loopback)
- **Streams** raw PCM audio data over TCP to a specified IP and port
- **Optimizes** bandwidth by detecting silence and skipping transmission when no audio is playing
- **Manages** multiple configuration profiles for different streaming scenarios

### How It Works

```
┌─────────────┐      ┌──────────────┐      ┌─────────────┐
│   Audio     │      │     TCP      │      │   Server    │
│   Input     │─────▶│   Streamer   │─────▶│ (Snapcast,  │
│  (Device)   │      │              │      │  Custom)    │
└─────────────┘      └──────────────┘      └─────────────┘
```

1. **Capture**: Reads audio from selected input device at configurable sample rate/buffer size
2. **Buffer**: Pushes audio into a lock-free **Ring Buffer** to absorb network jitter
3. **Process**: Analyzes RMS to detect silence
4. **Stream**: A separate network thread reads from buffer and sends data via Async TCP
5. **Monitor**: Real-time health monitoring (buffer usage, latency, bitrate)

---

## Features

### Core Functionality

- ✅ **Precision Audio Pacing** - Token Bucket algorithm for mathematically perfect transmission timing (v1.5.9+)
- ✅ **Robust Audio Engine** - Threaded architecture with Ring Buffer to prevent dropouts
- ✅ **Real-time Audio Streaming** - Low-latency PCM audio over TCP
- ✅ **Adaptive Buffer Sizing** - Automatically adjusts buffer based on network jitter
- ✅ **Silence Detection** - Smart bandwidth optimization with visual feedback
- ✅ **Auto-Reconnect** - Resilient connection management
- ✅ **Multi-Profile Support** - Save and switch between configurations
- ✅ **System Tray Integration** - Runs in background, accessible from tray
- ✅ **Windows Native Loopback** - Capture system audio without virtual cables (WASAPI)

### Audio Configuration

- 📊 **Sample Rates**: 44.1 kHz or 48 kHz
- 🔧 **Buffer Sizes**: 256, 512, 1024, or 2048 samples
- 🎤 **Input Devices**: Scans all host APIs (WASAPI, MME, CoreAudio) to find all devices
- 🎚️ **Visual Volume Indicator**: Real-time RMS meter for precise threshold tuning

### Automation

- 🚀 **Auto-start on Boot** - Launch automatically when system starts
- 🔄 **Auto-stream** - Begin streaming immediately on startup
- 🔒 **Auto-reconnect** - Retry connection on failure

### User Experience

- 🎨 **Modern UI** - Clean, icon-based tabbed interface
- � **Real-time Statistics** - Monitor bitrate, uptime, data sent
- �📶 **Network Quality Metrics** - Real-time health score (Excellent/Good/Fair/Poor)
- 📉 **Jitter Monitoring** - Live tracking of network stability
- 📝 **Activity Logs** - Track connection events, buffer resizes, and errors
- 🌙 **Minimize to Tray** - Never quits, always accessible

### Advanced Network Optimization

- ⚡ **Thread Priority** - High priority thread option for reduced jitter
- ⏱️ **Precision Pacer** - Drift-aware Token Bucket algorithm to eliminate micro-bursts
- 🎛️ **Network Presets** - One-click optimization for Ethernet/WiFi/Poor connections
- 🚦 **DSCP/TOS Support** - QoS tagging (VoIP, Low Delay, Throughput)
- 📦 **Dynamic Chunk Size** - Configurable buffer chunks (128-4096 samples)

---

## Use Cases

### 1. **Multi-Room Audio with Snapcast**

Stream audio from your computer to a Snapcast server, enabling synchronized playback across multiple rooms.

```bash
# Snapcast server listening on port 4953
TCP Streamer → 192.168.1.100:4953 → Snapcast Server → Multiple Speakers
```

### 2. **Remote Audio Monitoring**

Send audio from a security camera's microphone or monitoring device to a central server.

### 3. **Audio Distribution**

Distribute audio from a single source to multiple recipients via a TCP relay server.

### 4. **Development & Testing**

Test audio processing pipelines, codecs, or streaming protocols with a reliable audio source.

### 5. **Virtual DJ/Broadcast Setup**

Stream DJ mixes or live broadcasts from your computer to a remote server for distribution.

---

## Installation

### Download Pre-built Binaries

Download the latest release for your platform:

**[→ Releases Page](https://github.com/NaturalDevCR/TCP-Streamer/releases)**

| Platform    | File Type             | Installation                        |
| ----------- | --------------------- | ----------------------------------- |
| **macOS**   | `.dmg`                | Open and drag to Applications       |
| **Windows** | `.msi` or `.exe`      | Run installer                       |
| **Linux**   | `.AppImage` or `.deb` | Make executable or install via dpkg |

### Build from Source

#### Prerequisites

- [Node.js](https://nodejs.org/) (v18 or later)
- [Rust](https://www.rust-lang.org/) (latest stable)
- Platform-specific dependencies:
  - **Ubuntu/Debian**: `libgtk-3-dev libwebkit2gtk-4.0-dev libappindicator3-dev librsvg2-dev patchelf libasound2-dev`
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

> **Windows Loopback**: Check the **"Enable Loopback (Windows)"** box to capture system audio directly (no extra software needed, but you need to have speakers, or headphones plugged in to your computer in order to capture audio). or use software like VB Audio Cable to create a virtual audio device.
> **macOS Loopback**: Use software like [BlackHole](https://github.com/ExistentialAudio/BlackHole) or [Loopback](https://rogueamoeba.com/loopback/) to capture system audio.

### 2. **Configure Destination**

Enter the IP address and port of your TCP server:

- **IP**: `192.168.1.100` (example Snapcast server)
- **Port**: `4953` (Snapcast default TCP port)

### 3. **Adjust Settings**

- **Sample Rate**: 48 kHz (recommended) or 44.1 kHz
- **Buffer Size**: 1024 (balanced) or adjust for latency/stability

### 4. **Start Streaming**

Click **Start Streaming**. Monitor connection status and statistics in the UI.

### 5. **Automate** (Optional)

- Enable **Auto-start on launch** to run on system boot
- Enable **Auto-stream** to start streaming automatically
- Enable **Auto-reconnect** for resilient connections

---

## Configuration

### Configuration Profiles

Save different configurations for various scenarios:

1. **Create Profile**: Click **➕ New**, enter a name, click **✓ Create**
2. **Save Settings**: Adjust settings, click **💾 Save** to update current profile
3. **Switch Profile**: Select from dropdown to load saved configuration
4. **Delete Profile**: Select profile, click **🗑️ Delete** (cannot delete Default)

**Example Profiles:**

- `Home-Snapcast`: 192.168.1.100:4953, 48kHz, Auto-reconnect ON
- `Studio-Monitor`: 10.0.0.50:8000, 44.1kHz, High buffer
- `Testing`: localhost:9999, 48kHz, Low latency

### Audio Settings

| Setting         | Options              | Recommendation                       |
| --------------- | -------------------- | ------------------------------------ |
| **Sample Rate** | 44.1 kHz, 48 kHz     | 48 kHz for modern systems            |
| **Buffer Size** | 256, 512, 1024, 2048 | 1024 (balanced) or 512 (low latency) |
| **Ring Buffer** | 2000ms - 15000ms     | 4000ms for WiFi, 2000ms for Ethernet |

### Adaptive Buffer (New!)

Automatically resizes the ring buffer based on network conditions to prevent audio dropouts.

- **Enable**: Toggles the adaptive logic.
- **Min Buffer**: The floor value for the buffer (e.g., 2000ms).
- **Max Buffer**: The ceiling value (e.g., 10000ms).

_The system checks jitter every 10 seconds and adjusts the buffer size within these bounds._

#### Smart Buffer Sizing by Device Type

As of v1.5.3, the application automatically adjusts buffer settings based on device type:

**WASAPI Loopback (Windows)**:

- Base ring buffer: **8000ms** (handles WiFi jitter + laptop CPU throttling + WASAPI timing variability)
- Adaptive range: **4000-12000ms** (wider range for unpredictable conditions)
- Best latency: ~4 seconds (wired network, low jitter)
- Worst latency: ~12 seconds (WiFi laptop, high jitter)

**Standard Input/VB Audio Cable**:

- Base ring buffer: **5000ms** (WiFi tolerance)
- Adaptive range: **2000-6000ms** (more aggressive, tighter control)
- Best latency: ~2 seconds (wired network, low jitter)
- Worst latency: ~6 seconds (WiFi, high jitter)

**How it works**: The adaptive buffer uses real-time jitter measurements to dynamically adjust within these device-specific ranges. On stable wired networks, it shrinks toward the minimum. On WiFi or under load, it expands toward the maximum.

### Network Presets

Located in the **Advanced** tab, these presets configure multiple settings at once:

| Preset | Description | Settings Applied |
| ~ | ~ | ~ |
| **Ethernet** | For stable wired connections | Ring Buffer: 2s, Chunk: 512, Adaptive: 2s-6s |
| **WiFi** | For standard wireless | Ring Buffer: 4s, Chunk: 1024, Adaptive: 3s-10s |
| **WiFi (Poor)** | For unstable/far connections | Ring Buffer: 8s, Chunk: 2048, Adaptive: 5s-15s |

### Silence Detection

- **Visual Indicator**: Use the real-time volume bar to see your current audio level.
- **Threshold**: Set the slider just above the "noise floor" (white line).
- **Timeout**: Stop streaming after X seconds of silence to save bandwidth.

### Automation Settings

| Feature                  | Description                                       |
| ------------------------ | ------------------------------------------------- |
| **Auto-start on launch** | Launch app when system starts (minimized to tray) |
| **Auto-stream**          | Begin streaming immediately after app starts      |
| **Auto-reconnect**       | Retry connection every 3 seconds on failure       |

---

## Configuration Scenarios

### 1. High-Fidelity Music (Wired)

**Goal**: Lowest latency, highest stability.

- **Preset**: Ethernet
- **Sample Rate**: 48 kHz
- **Buffer Size**: 512
- **Adaptive Buffer**: Enabled (Min: 1000ms, Max: 4000ms)

### 2. Whole-Home Audio (WiFi)

**Goal**: Balanced stability for multi-room sync.

- **Preset**: WiFi
- **Sample Rate**: 44.1 kHz or 48 kHz
- **Buffer Size**: 1024
- **Adaptive Buffer**: Enabled (Min: 3000ms, Max: 10000ms)

### 3. Challenging Environment (Far from Router)

**Goal**: Prevent dropouts at all costs.

- **Preset**: WiFi (Poor Signal)
- **Sample Rate**: 44.1 kHz
- **Buffer Size**: 2048
- **Adaptive Buffer**: Enabled (Min: 5000ms, Max: 20000ms)

---

## System Requirements

### Minimum Requirements

- **OS**: macOS 10.15+, Windows 10+, Ubuntu 20.04+
- **RAM**: 100 MB
- **CPU**: Any modern processor
- **Network**: Stable network connection to TCP server

### Supported Platforms

- ✅ macOS (Intel & Apple Silicon)
- ✅ Windows 10/11
- ✅ Linux (Ubuntu, Debian, Fedora, Arch)

---

## Architecture

### Technology Stack

- **Frontend**: HTML, CSS, JavaScript (Vite)
- **Backend**: Rust (Tauri v2)
- **Audio**: cpal (cross-platform audio library)
- **Storage**: tauri-plugin-store (settings persistence)

### Audio Pipeline

```rust
Input Device → cpal → Producer → Ring Buffer → Consumer (Thread) → TCP Stream
```

- **Format**: Raw PCM, 16-bit signed integers, little-endian
- **Channels**: 2 (stereo)
- **Buffering**: Lock-free Ring Buffer (approx 2s capacity)
- **Silence Threshold**: RMS < 50.0 (skips transmission)

### Data Flow

1. **Capture**: cpal reads audio from device and pushes to **Ring Buffer** (Producer)
2. **Process**: Calculate RMS to detect silence before pushing
3. **Pacing**: **Precision Pacer** waits for the exact mathematically calculated time for the next chunk
4. **Transmit**: Dedicated **Network Thread** (Consumer) sends data via TCP without bursting
5. **Monitor**: Network thread calculates stats (bitrate, uptime) and emits events to UI

---

## Development

### Project Structure

```
tcp-streamer/
├── src/                  # Frontend (HTML/CSS/JS)
│   ├── index.html       # Main UI
│   ├── main.js          # Application logic
│   └── styles.css       # Styling
├── src-tauri/           # Backend (Rust)
│   ├── src/
│   │   ├── lib.rs       # App setup, tray, window management
│   │   └── audio.rs     # Audio streaming logic
│   └── Cargo.toml       # Rust dependencies
├── package.json         # Node dependencies
└── README.md
```

### Key Commands

```bash
# Development mode (hot reload)
npm run tauri dev

# Build for production
npm run tauri build

# Run tests
cargo test --manifest-path=src-tauri/Cargo.toml

# Format code
cargo fmt --manifest-path=src-tauri/Cargo.toml
npm run format
```

### Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

### Troubleshooting

### WASAPI Loopback Stuttering (Windows)

**Problem**: Audio stutters or has dropouts when using Windows WASAPI loopback

**Solutions**:

- WASAPI loopback is more sensitive to network conditions than VB Audio Cable
- The application automatically uses larger buffers (8000ms base) for loopback mode
- Enable **Adaptive Buffer** to handle varying network conditions
- On WiFi laptops, expect 4-12 seconds of latency (trade-off for stability)
- For wired networks, adaptive buffer will shrink toward 4 seconds
- Memory usage is minimal (~2.3MB max @ 12 seconds)

---

### Connection Issues

**Problem**: "Failed to connect" error

**Solutions**:

- Verify TCP server is running and listening on specified port
- Check firewall settings (allow TCP connections)
- Ensure IP address and port are correct
- Try `localhost` or `127.0.0.1` if server is on same machine

---

### Audio Device Not Found

**Problem**: Input device doesn't appear in dropdown

**Solutions**:

- **macOS**: Grant microphone permissions in System Settings → Privacy & Security
- **Windows**: Check audio device is enabled in Sound settings
- **Linux**: Ensure ALSA/PulseAudio is configured correctly
- Restart the application after connecting new audio devices

---

### No Audio Streaming (Silence Detection)

**Problem**: Connection established but no audio is being sent

**Possible Causes**:

- Silence detection is working correctly (no audio playing)
- Audio input device is muted or volume is too low
- Wrong input device selected

**Solutions**:

- Play audio on your computer while streaming
- Check input device volume/mute status
- Try a different input device
- Check logs for "Silence detected" (RMS values)

---

### High CPU Usage

**Problem**: Application uses excessive CPU

**Solutions**:

- Increase **Buffer Size** to 2048 (reduces processing frequency)
- Lower **Sample Rate** to 44.1 kHz if 48 kHz is unnecessary
- Disable unused features

---

### Permission Errors (macOS)

**Problem**: "App is damaged and can't be opened"

**Solution**:

```bash
# Remove quarantine attribute
xattr -cr /Applications/TCP\ Streamer.app
```

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## Support

- **Issues**: [GitHub Issues](https://github.com/NaturalDevCR/TCP-Streamer/issues)
- **Discussions**: [GitHub Discussions](https://github.com/NaturalDevCR/TCP-Streamer/discussions)

---

## Support the Project

If you find TCP Streamer useful and would like to support its development, consider making a donation:

[![Donate with PayPal](https://img.shields.io/badge/Donate-PayPal-blue.svg)](https://paypal.me/NaturalCloud)

Your support helps maintain and improve this project. Thank you! 🙏

---

## Acknowledgments

- Built with [Tauri](https://tauri.app/)
- Audio library: [cpal](https://github.com/RustAudio/cpal)
- Inspired by multi-room audio systems like [Snapcast](https://github.com/badaix/snapcast)

## Notice
For anyone concerned, this project was vibe-coded (with some personal tweaks) using Gemini 3 Pro and Sonnet 4.5, I did this, just because I needed something specific to send audio from computers to a Snapcast server, it started simple, but it beceame more complex as I was realizing there are many things to optimize and many scenarios... so, while this is a work in progress, so far it's working well.

---

**Made with ❤️ for the audio streaming community**
