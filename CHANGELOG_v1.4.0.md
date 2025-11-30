# TCP-Streamer v1.4.0 Release Notes

## 🎛️ New Features

### Dual RMS Volume Metrics

- **Real-time VU Meter**: Smoothed visual indicator with fast attack (0.5) and slow decay (0.05) for natural, stable feedback
- **Signal Average**: Intelligent average that only updates during active audio, ignoring silence gaps between songs
- Both metrics displayed side-by-side in the UI for better volume monitoring

### Dynamic Silence Detection

- **Live Settings Updates**: Silence threshold and timeout now update in real-time without requiring stream restart
- **Instant Feedback**: Change settings and see immediate results in audio transmission behavior
- New Tauri command `update_silence_settings` enables on-the-fly configuration

## 🔧 Improvements

### Windows Loopback Compatibility

- **Better Device Support**: Loopback mode now uses `BufferSize::Default` for improved compatibility with internal laptop speakers and integrated audio devices
- **Reduces Connection Errors**: Fixes "Failed to connect to device" errors on many Windows systems
- External audio devices (USB, headphones) continue to work as before

### User Experience

- **Disabled by Default**: Silence detection threshold and timeout now default to 0 (disabled) to prevent confusion for new users
- **Clear Visual Feedback**: Average volume display helps users set appropriate silence thresholds

## 🐛 Bug Fixes

- Fixed silence detection not applying when settings were changed during active streaming
- Resolved duplicate function definitions in audio processing code
- Corrected variable scoping issues in silence detection state management

## 📦 Technical Details

- Implemented global atomic variables (`SILENCE_THRESHOLD_BITS`, `SILENCE_TIMEOUT_SECS`) for lock-free real-time updates
- Added `VolumePayload` struct for structured volume event emission
- Enhanced HTML UI with dual volume display (`volume-value` and `avg-volume-value`)
- Improved loopback stream configuration logic for WASAPI compatibility

## 🙏 Acknowledgments

Special thanks to all users testing and providing feedback on the Windows loopback functionality and silence detection features.

---

**Full Changelog**: https://github.com/NaturalDevCR/TCP-Streamer/compare/v1.3.0...v1.4.0
