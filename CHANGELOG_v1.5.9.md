# Changelog v1.5.9

## New Features

- **Precision Audio Pacing (Phase 1)**: Implemented a Token Bucket algorithm in the network thread to regulate audio transmission timing. This ensures audio chunks are sent at mathematically precise intervals (e.g., exactly every 21.33ms for 1024-sample chunks at 48kHz), eliminating network micro-bursts and significantly reducing jitter on the receiver side.
- **Drift Correction**: Added logic to automatically reset the pacer if the stream falls significantly behind (e.g., after a pause or network blackout) to prevent "fast-forwarding" old audio.

## Improvements

- **Documentation**: Updated README and in-app documentation to explain the new pacing engine and its benefits.
- **Stability**: Smoother playback on Snapcast clients due to regular packet arrival times.

## Fixes

- Addressed potential buffer overflow reporting accuracy.
