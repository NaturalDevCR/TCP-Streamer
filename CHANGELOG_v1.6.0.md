# Changelog v1.6.0

## New Features

- **Smart Silence Detection**:

  - **Bandwidth Saver**: Automatically stops sending audio packets when RMS volume drops below the threshold, saving network bandwidth.
  - **Deep Sleep Mode**: Automatically disconnects the TCP stream if silence persists for a configurable duration (default: disabled/0).
  - **Auto-Resume**: Instantly reconnects and resumes streaming when audio activity is detected.

- **Connection Reliability**:

  - **Write Timeout**: Added a 5-second write timeout to the TCP socket. This ensures the application quickly detects dead connections (e.g., during computer sleep/wake or network drops) instead of hanging indefinitely.
  - **Graceful Shutdown**: Intercepted application exit events to ensure the TCP connection is closed properly (sending a FIN packet) before the process terminates, preventing "zombie" connections on the server.

- **Smart Defaults**:
  - **Silence Timeout**: Set to 0 (Disabled) by default to prevent unexpected disconnections for new users.

## Improvements

- **Resource Management**: Reduced CPU usage during silence by skipping packet creation.
- **Server Health**: Significantly reduced the likelihood of `FIN-WAIT` or stuck connections on the Snapserver side.

## Fixes

- Fixed an issue where the application would hang if the network disconnected while streaming.
- Fixed an issue where closing the app would leave the server connection open until it timed out.
