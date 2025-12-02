# Changelog v1.5.6

## 🚀 Performance Improvements

### Batch Processing for Network Thread

Implemented a significant performance optimization to address persistent buffer overflow issues on specific hardware configurations (including Linux/Zorin OS laptops).

#### The Issue

Even with the `yield_now()` fix in v1.5.5, the network thread was processing audio chunks one by one. On systems with higher latency or specific scheduling characteristics, the thread couldn't drain the ring buffer fast enough when audio data accumulated, leading to eventual overflows.

#### The Fix

Introduced **Batch Processing** in the network thread.

- The thread now attempts to drain up to **10 chunks** of audio data in a single loop iteration before yielding.
- This allows the network thread to "catch up" much faster if it falls behind the audio producer.
- Significantly improves throughput and resilience against system latency spikes.

#### Technical Details

- **File**: `src-tauri/src/audio.rs`
- **Change**: Replaced single-chunk processing with a batch loop (max 10 chunks).
- **Impact**: Eliminates buffer overflows on systems where single-chunk processing was insufficient.

## 🔧 Development

- **Build**: Release (optimized)
- **Date**: 2025-12-02
