# Changelog

All notable changes to TCP Streamer are documented in this file.

---

## [1.9.5] - 2026-03-14
### Bug Fixes
- **Latency tracking**: Replaced `Vec::remove(0)` (O(n)) with `VecDeque::pop_front()` (O(1)) for latency sample tracking.
- **Error counting**: `error_count` in quality metrics is now tracked and emitted instead of being hardcoded to `0`.
- **Buffer size display**: Fixed ring buffer MB calculation — was using `×2` (i16 bytes) instead of `×4` (f32 bytes).
- **Socket safety**: Socket creation no longer panics on failure — gracefully logs the error and retries.
- **Format comment**: Fixed `AudioCommand::format` comment from "pcm or mp3" to "pcm or wav".

### Code Quality
- **StreamParams struct**: Replaced 17-element reconnection tuple with a named `StreamParams` struct.
- **Dead code removal**: Removed `_reconnect_handle`, `sequence`, `_last_write_time`, `data_len`, and all MP3 encoder commented code.
- **Unused parameter**: `buffer_size` parameter removed from internal pipeline (frontend still sends it for compatibility).
- **Copy type clones**: Fixed unnecessary `.clone()` calls on primitive/Copy types in reconnection logic.
- **Stale comments**: Cleaned all `// protocol removed`, `// UDP removed`, `// MP3 removed` markers.
- **Typo fix**: "Gloabl" → "Global" in stats.rs rate limiter comment.

## [1.9.4] - 2026-03-14
### Improvements
- **Native Stereo Pipeline**: Audio is now captured and streamed in full stereo (2 channels) preserving complete L/R separation. No more downmixing to mono.
- **Channel-aware pipeline**: Ring buffer, pacing, prefill, drain, and WAV header all dynamically adapt to the detected device channel count.
- **Cleaner architecture**: Format and channel detection moved earlier in the pipeline for better code organization.

## [1.9.3] - 2026-03-14
### Bug Fixes
- **Fix chipmunk audio**: Added stereo-to-mono downmix in the CPAL audio capture callback. Devices capturing in stereo (2 channels) were pushing interleaved L/R samples directly into the mono pipeline, causing audio to play at 2x speed.
- **Loopback format detection**: Loopback devices now correctly query `supported_output_configs()` instead of `supported_input_configs()`.

## [1.9.2] - 2026-03-10
### Bug Fixes
- **Restore multi-format CPAL stream builder**: Restored dynamic `SampleFormat` selection (F32, I16, U16) which was lost during the v1.9.1 refactor. Devices that natively capture at 16-bit were silently failing, resulting in empty ring buffers (`Buffering... 0/9600 samples`).

## [1.9.1] - 2026-03-07
### Changes
- **Linux post-install script**: Added deb post-install script to automatically set `CAP_SYS_NICE` for real-time thread priority.
- **Linux build fix**: Removed invalid `tauri::WebviewWindowExt` import that broke Linux builds.

## [1.9.0] - 2026-03-07
### Features
- **Client/Server modes**: Added TCP Server mode (listen for incoming connections) alongside existing Client mode (connect to remote server).
- **HTTP streaming support**: Server mode auto-detects HTTP clients (browsers, VLC) and serves `audio/wav` with chunked transfer encoding.
- **Connection resilience**: Improved reconnection logic with exponential backoff and jitter.
- **Code refactor**: Split monolithic `audio.rs` into modular `audio/manager.rs`, `audio/commands.rs`, `audio/stats.rs`, `audio/stream.rs`, and `audio/wav_helper.rs`.

## [1.8.9] - 2026-03-07
### Bug Fixes
- **Adaptive buffer drain**: Added overflow prevention by draining excess samples when buffer fills during disconnection periods.

## [1.8.8] - 2026-03-07
### Bug Fixes
- **Strict clock loop**: Implemented strict pacing with silence padding to prevent clock drift and timing-related audio artifacts.

## [1.8.7]
### Maintenance
- Removed accidental `pnpm-lock.yaml` from repository.

## [1.8.6]
### Bug Fixes
- **Linux CI**: Added `libfuse2` dependency for `linuxdeploy` AppImage support.

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
- **Prefill Gate**: Added 1000ms audio prefill before transmission starts to prevent cold-start stuttering.
- **Volume fix**: Corrected volume levels in audio processing.

## [1.8.0]
### Features
- **F32 Audio Engine**: Upgraded internal audio processing from I16 to F32 for higher fidelity and dynamic range.

## [1.7.0]
### Features
- **High Precision Pacing**: Implemented spin-loop based high-precision pacing for ultra-low jitter audio delivery.

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
- **Critical AutoSync Fix**: Resolved AutoSync timing issues that caused stream desynchronization.
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
- **Active Rate Control**: Implemented dynamic rate control for smoother streaming.
- **Auto Sync**: Added automatic synchronization between sender and receiver.

## [1.6.0]
### Features
- **Smart Silence Detection**: Intelligent silence detection to reduce bandwidth during quiet periods.
- Reliability fixes for long-running streams.

## [1.5.9]
### Improvements
- Precision pacing improvements for timing accuracy.

## [1.5.8]
### Bug Fixes
- **Graceful TCP shutdown**: Proper FIN/ACK sequence to prevent zombie TCP connections on stop.

## [1.5.7]
### Improvements
- UI improvements and platform-specific fixes.

## [1.5.6]
### Performance
- **Batch processing**: Implemented batch processing in network thread for reduced CPU usage and improved throughput.

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
- **Dual RMS metrics**: Separate input/output RMS level monitoring.
- **Dynamic silence detection**: Adaptive silence threshold based on ambient noise.
- Improved Windows WASAPI loopback reliability.

## [1.3.0]
### Features
- **Adaptive Buffer**: Dynamic ring buffer sizing based on network jitter.
- **Quality Metrics**: Real-time quality score (0-100) based on jitter, buffer health, and errors.
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
### 🎉 Initial Stable Release
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
