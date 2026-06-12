# Changelog

All notable changes to TCP Streamer are documented in this file.

---

## [2.3.0] — 2026-06-12

### UI/UX — Phase 3 Redesign

- **Enterprise dark theme** with premium glassmorphism design (Vue 3 + Tailwind CSS 4 + reka-ui).
- **Top navigation** replaced sidebar: Dashboard, Connection, Audio, Logs, Settings sections.
- **Full i18n (en/es)** with vue-i18n: all labels, status messages, and contextual help translated.
- **Keyboard-accessible tooltips** (`SettingLabel`) explaining every configurable option in Connection, Audio, and Settings.
- **New UI component system:** Badge, Button, Card, CopyField, Dialog, Field, Input, Select, SegmentedControl, SettingLabel, Stat, StatusDot, Switch, Textarea, Tooltip.
- **Dashboard** with real-time stats: uptime, bitrate, transferred bytes, RTT, underruns, quality score.
- **Recent logs** panel on Dashboard; log viewer with level filter and search.

### Audio Engine

- **Sink / playback role** — Bidirectional operation: Source captures + sends; Sink receives + plays to a local output device.
- **Wire format normalized** to fixed stereo s16le at the configured output rate; internal resampling handles any capture device.
- **Catmull-Rom resampler** for high-quality sample-rate conversion between device rate and wire rate.
- **Mono, stereo & multichannel capture** — Mono duplicated to stereo; multichannel uses front left/right pair.
- **i32 capture support** in addition to f32, i16, and u16.
- **Sink output negotiation** — Automatic format, channel, and sample-rate conversion for playback devices (f32, i16, u16, i32).
- **Frame-aligned corrections** — Buffer drops, catch-up, and drift corrections preserve L/R channel alignment.

### Connectivity — Phase 2A

- **Configurable latency profiles** (Ultra-low, Balanced, Robust, Custom) with tested ring buffer and chunk parameters.
- **IP/CIDR allowlist** for TCP server mode — restrict access to specific addresses or networks.
- **DNS/hostname resolution** for client connections (resolve hostnames in target address).
- **IPv6 & dual-stack** — Server binds on dual-stack IPv6; client connects via resolved addresses.

### Native UDP — Phase 2B

- **Native UDP transport** — Low-latency streaming with sequence/timestamp framing and de-jitter buffer.
- **Loss concealment** — Missing packets concealed to avoid audible gaps.
- **mDNS discovery** — Sources advertise on the LAN; sinks scan and auto-populate the source list.
- **Optional AEAD/PSK encryption** — Per-packet ChaCha20-Poly1305 with HKDF-SHA256 key derivation; replay-protected.
- **Clock-drift handling** — Occasional mini-chunk insert/drop to track long-run clock differences (no PTP required).

### Foundation — Phase 1

- **TypeScript migration** — Full JS → TS conversion for stores, composables, types, and i18n.
- **Package manager** migrated to pnpm; Prettier formatting; Lefthook pre-commit hooks.
- **Rust module decomposition** — `engine/` (buffer, capture, convert, decoder, device, encoder, latency, pacing, playback, sink) and `transport/` (allowlist, discovery, dscp, resolve, tcp_client, tcp_server, udp).
- **Real adaptive buffer controller** — Grows standing-latency target on glitch, shrinks when stable (was cosmetic before).
- **Honest metrics** — Real underrun/overrun counters + RTT via TCP_INFO; QualityEvent with rtt_ms, underruns, dropped.
- **Effective CPAL buffer size** — Hardware buffer validated against device-supported range (was silently ignored).
- **Real-time-safe capture** — No mutex or per-callback heap allocation; overrun counter.
- **DSCP/QoS fixed** — Tested strategy→TOS mapping applied in both client and server modes.
- **Connection trait** — Abstracts TCP/UDP behind common Write + RTT interface; engine is transport-agnostic.
- **Single reconnection policy** — Consolidated two competing mechanisms; honors Auto-Reconnect toggle.
- **Bounded HTTP handshake** — Non-blocking detection (removes 1.5s stall on new connections).
- **Engine orchestrator** moved from manager.rs to `engine::run`; `stream.rs` enum deleted.

### Fixed

- Fixed chipmunk/choppy audio from capture-device channel count and sample rate reaching the wire unchanged.
- Aligned Tauri JS API with Rust runtime; installed required Linux native deps in CI.

## [2.2.0] — 2026-06-06

### Added

- **Sink / playback role** — Receive and play audio to a local output device (bidirectional: Source or Sink).
- **Native UDP mode** — Low-latency UDP transport with de-jitter buffer (sequence/timestamp framing, loss concealment).
- **Optional AEAD/PSK encryption** — Per-packet ChaCha20-Poly1305 with HKDF-SHA256 key derivation; authenticated and replay-protected.
- **mDNS discovery** — Sources advertise on LAN; sinks scan and pick them (manual entry still available).
- **Configurable latency profiles** — Ultra-low, Balanced, Robust, Custom replacing old network presets.
- **IPv6 & hostname support** — Client connect via resolution; dual-stack server bind.
- **IP/CIDR allowlist** for server mode.
- **Clock-drift handling** on sink (no PTP): occasional mini-chunk drop/insert to track long-run drift.

### Changed

- Generic terminology throughout (works with any audio receiver; no vendor-specific naming).

## [2.1.0] — 2026-06-06

### Added

- **Real adaptive buffer** — Target-occupancy controller that grows buffered latency when glitches occur and shrinks toward the minimum when stable (previously cosmetic).
- **Honest metrics** — Real underrun/overrun counters and best-effort TCP RTT via `TCP_INFO` (Linux/macOS) replace misleading `write()`-timing figures. Stats bar shows RTT and Underruns.
- **Effective capture buffer size** — Buffer Size control now sets CPAL hardware buffer (`BufferSize::Fixed`, validated against device range) instead of being silently ignored.
- **Audio engine decomposition** into tested modules: `engine/{device,encoder,capture,buffer}`, `transport/{dscp,tcp_client,tcp_server}` behind `Connection` trait, and `metrics`. Rust unit tests: 10 → 40.

### Fixed

- **DSCP/QoS** — Strategy dropdown now maps to correct IP TOS bytes (previously no UI value matched backend; QoS never applied). Now applied in both client and server modes.
- **Real-time-safe audio callback** — Removed `Arc<Mutex>` lock and per-callback heap allocation from CPAL capture callback, eliminating priority-inversion/dropout risk.
- **HTTP handshake** no longer blocks the network thread for up to 1.5 s on each new connection.
- Consolidated two competing reconnection mechanisms into a single policy that honors Auto-Reconnect toggle.

### Changed

- `QualityEvent` IPC payload reshaped to honest fields (`rtt_ms`, `rtt_var_ms`, `underruns`, `dropped`).
- UI labels distinguish capture buffer from jitter buffer.
- Package manager migrated to **pnpm**.

## [2.0.6] — 2026-03-15

### Fixed

- **Settings Store Error** — Fixed store values (e.g., `loopbackMode`) resolving to `undefined` during `store.set`, which triggered a "missing required key value" crash on startup.

## [2.0.5] — 2026-03-14

### Added

- **Dynamic Window Resize** — Auto-detects host screen resolution at launch and bounds initial height dynamically (1000px max, 150px margin) to prevent window sinking behind OS taskbars on smaller laptops.

### Fixed

- **UI Header Scroll** — Extracted Top Header, Status Bar, and Tab Navigation out of the scrollable region; permanently pinned to window top.

## [2.0.4] — 2026-03-14

### Fixed

- **UI Overlap & Scrolling** — Refactored `App.vue` layout. Main viewport and bottom Action Bar strictly separated via Flexbox (`flex-1` vs `shrink-0`); footer no longer floats behind Start button; scrollbar reaches end of content.

## [2.0.3] — 2026-03-14

### Fixed

- **UI Scrolling** — Restored `h-screen` bounding box after `h-full` inadvertently disabled scrollbar and clipped bottom of long tabs.
- **Native Decorations** — Enforced `"transparent": false` in Tauri config to guarantee GNOME/Wayland compositor renders native control buttons.
- **Jitter Formula** — Replaced CPU interval deviation with TCP transmit standard deviation, showing correct 0.0–0.2ms jitter for stable LAN instead of misleading 16.7ms driver buffer slices.

## [2.0.2] — 2026-03-14

### Fixed

- **Linux WebKitGTK Decorations** — Removed programmatic `set_decorations(false)` override that stripped native window controls on GNOME/Wayland.
- **Jitter Formula** — Same TCP transmit stddev fix as v2.0.3.

## [2.0.1] — 2026-03-14

### Fixed

- **Audio Stuttering (Underflow)** — Replaced strict CPU clock-based pacing with hardware-driven ring buffer pacing in the network thread. Eliminates artificial underflows when CPU and audio hardware are out of sync.
- **Audio Resampling** — Removed forced 44.1kHz downsampling. App now relies on native OS audio stack implicit resampling, preserving user's requested sample rate without chipmunk effects.
- **Linux Window Overlap** — Removed custom Vue titlebar overlapping with native DE window decorations.
- **Window Size** — Reduced default Linux window height from 1000px to 750px.

## [2.0.0] — 2026-03-14

### Added

- **Complete UI Redesign** — Migrated from Vanilla JS to Vue 3, Pinia, and Tailwind CSS 4.
- **Modern Interface** — Dark glassmorphism theme with premium components, layout, and animations.
- **Enhanced UI Smoothness** — Native View Transitions API for seamless tab switching; refined cubic-bezier physics for inputs, checkboxes, and buttons.
- **New App Logo** — Custom TCP Streamer branding; new desktop app icons.
- **Tabbed Layout** — Organized settings into Connection, Audio, Settings, Advanced, and Logs tabs.

## [1.9.6] — 2026-03-14

### Bug Fixes

- **Auto-Reconnection** — Fixed critical bug where reconnection never triggered after connection loss. Command receiver (`mpsc::Receiver`) was blocking the thread; replaced with `try_recv` polling loop.
- **Buffer Resize Events** — Fixed event name mismatch (frontend listened for `buffer-resize-event`, backend emitted `buffer-resize`).
- **I16 Audio Format** — Fixed decode/encode asymmetry for I16 sample conversion (divisor 32768→32767 to match encoder).
- **Window Close Safety** — Replaced `window.hide().unwrap()` with safe `.ok()`.

### Code Quality

- **stream.rs** — Cleaned stale comments from single-variant enum.
- **main.js** — Removed duplicate `minBufferInput` assignment and duplicate `sampleRateSelect.disabled` lines.
- **Adaptive buffer** — Added clarifying comment that adjustments are display-only (ring buffer fixed at creation).

## [1.9.5] — 2026-03-14

### Bug Fixes

- **Latency tracking** — Replaced `Vec::remove(0)` (O(n)) with `VecDeque::pop_front()` (O(1)).
- **Error counting** — `error_count` now tracked and emitted instead of hardcoded to 0.
- **Buffer size display** — Fixed ring buffer MB calculation (was using i16 bytes×2 instead of f32 bytes×4).
- **Socket safety** — Socket creation no longer panics on failure; gracefully logs error and retries.
- **Format comment** — Fixed `AudioCommand::format` comment from "pcm or mp3" to "pcm or wav".

### Code Quality

- **StreamParams struct** — Replaced 17-element reconnection tuple with named struct.
- **Dead code removal** — Removed `_reconnect_handle`, `sequence`, `_last_write_time`, `data_len`, all MP3 encoder commented code.
- **Unused parameter** — `buffer_size` parameter removed from internal pipeline (frontend still sends it for compat).
- **Copy type clones** — Fixed unnecessary `.clone()` calls on primitive/Copy types.
- **Stale comments** — Cleaned all `// protocol removed`, `// UDP removed`, `// MP3 removed` markers.
- **Typo fix** — "Gloabl" → "Global" in stats.rs rate limiter comment.

## [1.9.4] — 2026-03-14

### Improvements

- **Native Stereo Pipeline** — Audio captured and streamed in full stereo (2 channels) preserving complete L/R separation. No more downmixing to mono.
- **Channel-aware pipeline** — Ring buffer, pacing, prefill, drain, and WAV header dynamically adapt to detected device channel count.
- **Cleaner architecture** — Format and channel detection moved earlier in the pipeline.

## [1.9.3] — 2026-03-14

### Bug Fixes

- **Fix chipmunk audio** — Added stereo-to-mono downmix in CPAL capture callback. Stereo devices were pushing interleaved L/R directly into mono pipeline, causing 2x speed playback.
- **Loopback format detection** — Loopback devices now correctly query `supported_output_configs()` instead of `supported_input_configs()`.

## [1.9.2] — 2026-03-10

### Bug Fixes

- **Restore multi-format CPAL stream builder** — Restored dynamic `SampleFormat` selection (F32, I16, U16) lost during v1.9.1 refactor. Devices natively capturing at 16-bit were silently failing, resulting in empty ring buffers.

## [1.9.1] — 2026-03-07

### Changes

- **Linux post-install script** — Added deb post-install script to automatically set `CAP_SYS_NICE` for real-time thread priority.
- **Linux build fix** — Removed invalid `tauri::WebviewWindowExt` import that broke Linux builds.

## [1.9.0] — 2026-03-07

### Features

- **Client/Server modes** — Added TCP Server mode (listen for incoming connections) alongside existing Client mode (connect to remote server).
- **HTTP streaming support** — Server mode auto-detects HTTP clients (browsers, VLC) and serves `audio/wav` with chunked transfer encoding.
- **Connection resilience** — Improved reconnection logic with exponential backoff and jitter.
- **Code refactor** — Split monolithic `audio.rs` into modular `audio/manager.rs`, `audio/commands.rs`, `audio/stats.rs`, `audio/stream.rs`, and `audio/wav_helper.rs`.

## [1.8.9] — 2026-03-07

### Bug Fixes

- **Adaptive buffer drain** — Added overflow prevention by draining excess samples when buffer fills during disconnection periods.

## [1.8.8] — 2026-03-07

### Bug Fixes

- **Strict clock loop** — Implemented strict pacing with silence padding to prevent clock drift and timing-related audio artifacts.

## [1.8.7]

### Maintenance

- Removed accidental `pnpm-lock.yaml` from repository.

## [1.8.6]

### Bug Fixes

- **Linux CI** — Added `libfuse2` dependency for `linuxdeploy` AppImage support.

## [1.8.5]

### Bug Fixes

- Fixed transmission start issues causing initial silence or delays.

## [1.8.4]

### Security

- Security hardening and vulnerability fixes.

## [1.8.3]

### Maintenance

- Regenerated application icons.

## [1.8.2]

### Maintenance

- Version bump and minor fixes.

## [1.8.1]

### Bug Fixes

- **Prefill Gate** — Added 1000ms audio prefill before transmission starts to prevent cold-start stuttering.
- **Volume fix** — Corrected volume levels in audio processing.

## [1.8.0]

### Features

- **F32 Audio Engine** — Upgraded internal audio processing from I16 to F32 for higher fidelity and dynamic range.

## [1.7.0]

### Features

- **High Precision Pacing** — Implemented spin-loop based high-precision pacing for ultra-low jitter audio delivery.

## [1.6.6]

### Features

- Snapcast features toggle in UI.
- Robust timestamp validation.
- Updated documentation.

## [1.6.5]

### Bug Fixes

- Fixed jitter calculation accuracy and UI precision display.

## [1.6.4]

### Bug Fixes

- **Critical AutoSync Fix** — Resolved AutoSync timing issues that caused stream desynchronization.
- Added log filters for cleaner output.

## [1.6.3]

### Features

- Real-time Jitter/Latency metrics display.
- Audio engine optimizations.

## [1.6.2]

### Bug Fixes

- Fixed compilation errors introduced in v1.6.1.

## [1.6.1]

### Features

- **Active Rate Control** — Implemented dynamic rate control for smoother streaming.
- **Auto Sync** — Added automatic synchronization between sender and receiver.

## [1.6.0]

### Features

- **Smart Silence Detection** — Intelligent silence detection to reduce bandwidth during quiet periods.
- Reliability fixes for long-running streams.

## [1.5.9]

### Improvements

- Precision pacing improvements for timing accuracy.

## [1.5.8]

### Bug Fixes

- **Graceful TCP shutdown** — Proper FIN/ACK sequence to prevent zombie TCP connections on stop.

## [1.5.7]

### Improvements

- UI improvements and platform-specific fixes.

## [1.5.6]

### Performance

- **Batch processing** — Implemented batch processing in network thread for reduced CPU usage and improved throughput.

## [1.5.5]

### Bug Fixes

- Fixed buffer overflow caused by CPU contention under high load.

## [1.5.4]

### Improvements

- Enhanced error logging for stream debugging.

## [1.5.3]

### Improvements

- WASAPI Loopback buffer improvements for Windows audio capture stability.

## [1.5.2]

### Bug Fixes

- Fixed silence timeout logic causing premature stream termination.

## [1.5.1]

### Bug Fixes

- Critical fixes and stability improvements.

## [1.5.0]

### Features

- UI improvements and comprehensive documentation overhaul.

## [1.4.0]

### Features

- **Dual RMS metrics** — Separate input/output RMS level monitoring.
- **Dynamic silence detection** — Adaptive silence threshold based on ambient noise.
- Improved Windows WASAPI loopback reliability.

## [1.3.0]

### Features

- **Adaptive Buffer** — Dynamic ring buffer sizing based on network jitter.
- **Quality Metrics** — Real-time quality score (0-100) based on jitter, buffer health, and errors.
- UI improvements for monitoring stream health.

## [1.2.0]

### Features

- Added PayPal donation button.

## [1.1.0]

### Improvements

- Improved loopback device matching on Windows.
- Verbose logging for debugging device selection.
- UI fixes.

## [1.0.2]

### Bug Fixes

- Forced boolean type for `isLoopback` argument to fix Tauri command deserialization.

## [1.0.1]

### Bug Fixes

- Resolved log spam by throttling repetitive messages.
- Disabled silence timeout by default to prevent unexpected disconnections.

## [1.0.0]

### Initial Stable Release

- TCP audio streaming from any input device or WASAPI loopback.
- Configurable sample rate, buffer size, and chunk size.
- Auto-reconnection with exponential backoff.
- Cross-platform support: Windows, Linux, macOS.
- Built with Tauri 2 + Rust + CPAL.

## [0.9.x]

### Pre-release

- Loopback checkbox UI.
- Snake_case/camelCase argument fixes for Tauri compatibility.
- WASAPI loopback support (Windows).
- Initial device selection and streaming logic.

## [0.8.x]

### Pre-release

- Enhanced metadata and smart reconnection.
- Removed Snapcast headers to fix audio distortion.
- UI cleanup and code refinement.
