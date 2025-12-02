# Changelog v1.5.7

## ✨ UI/UX Improvements

### Platform-Specific UI Enhancements

Improved the user interface to be more platform-aware and contextual.

#### Windows-Only Loopback UI

- **Change**: The "Enable Loopback (Windows)" checkbox now automatically hides on Linux and macOS
- **Impact**: Cleaner interface on non-Windows platforms, reducing user confusion about unavailable features
- **Implementation**: Added OS detection via new `get_os_type` command

#### Silence Detection UI Toggle

- **Change**: Silence threshold and timeout controls now hide automatically when "Disable Silence Detection" is checked
- **Impact**: Cleaner interface when silence detection is not in use
- **Behavior**: Toggle the checkbox to show/hide the advanced controls

---

## 🔧 Platform-Specific Fixes

### Linux Device Filtering

Fixed the issue where Linux showed dozens of non-functional audio devices in the device list.

#### The Issue

Linux ALSA exposes many virtual devices (`default`, `sysdefault`, `null`, `hw:*`) that don't work reliably for real-time audio streaming, cluttering the device selection dropdown.

#### The Fix

Implemented smart filtering that excludes ALSA virtual devices and only shows:

- Real hardware devices (e.g., "USB Audio Device")
- `plughw:*` devices (ALSA plugin devices that work reliably)

**Filtered out:**

- `default` / `sysdefault` - System default redirects
- `null` - Dummy device
- `hw:*` - Direct hardware access (unreliable timing)

---

## 🚨 Enhanced Diagnostics

### Server Slow Response Alert

Added automatic detection of slow server or network responses.

#### How It Works

- Monitors network write duration in real-time
- Triggers warning when writes take longer than **200ms**
- Helps distinguish between local buffer issues and server/network problems

#### Alert Format

```
⚠️ Server slow response: 245.3ms (network congestion or server overload)
```

**Use Case**: When you see buffer overflows AND slow server warnings, the issue is likely on the Snapserver or network side, not your client configuration.

---

## 🔧 Technical Details

**Files Modified:**

- `src-tauri/src/audio.rs` - Device filtering, slow server detection, OS detection
- `src-tauri/src/lib.rs` - Registered `get_os_type` command
- `src/main.js` - Platform detection and UI toggle logic
- `src/index.html` - UI containerization for show/hide

## 🔧 Development

- **Build**: Release (optimized)
- **Date**: 2025-12-02
