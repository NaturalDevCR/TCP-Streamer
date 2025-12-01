# Changelog v1.5.5

## 🐛 Bug Fixes

### Critical Buffer Overflow Fix

Fixed a critical issue where the audio stream would stop or drop packets continuously due to buffer overflows, even on high-quality networks.

#### The Issue

The network thread was consuming CPU resources too aggressively (busy-waiting) when processing audio data, causing contention with the audio callback thread. This prevented the audio system from writing data to the ring buffer efficiently, leading to overflows despite having large buffer sizes (e.g., 8000ms).

#### The Fix

Implemented `thread::yield_now()` in the network loop to voluntarily yield CPU time to other threads (specifically the audio callback) after processing each data chunk. This ensures smoother coordination between audio capture and network transmission.

#### Technical Details

- **File**: `src-tauri/src/audio.rs`
- **Change**: Added `thread::yield_now()` after successful network writes.
- **Impact**: Eliminates buffer overflows caused by CPU contention, ensuring stable streaming even with high sample rates (48kHz) and small chunk sizes.

## 🔧 Development

- **Build**: Release (optimized)
- **Date**: 2025-12-01
