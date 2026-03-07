#!/bin/sh
# This script runs after the .deb package is installed.
# Grant CAP_SYS_NICE so the app can elevate network thread priority without root.
setcap cap_sys_nice+ep /usr/bin/tcp-streamer || echo "Warning: Failed to set CAP_SYS_NICE on tcp-streamer"
exit 0
