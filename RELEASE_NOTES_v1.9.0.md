# 🛡️ TCP Streamer v1.9.0 - Connection Resilience

This release fixes a critical issue that caused **connection storms** when the network briefly dropped, which crashed Snapserver on 2026-01-29.

## 🔥 What Happened

When the network dropped for "Otocuma-Bar", the app opened **10 simultaneous connections** in 2 minutes. This triggered the server's watchdog to send a fatal signal (SIGUSR1), killing the audio server and silencing the entire hotel.

## ✅ What's Fixed

### 1. Enhanced Exponential Backoff

- Minimum reconnection delay increased from **1s → 2s**
- Added **random jitter (±500ms)** to prevent multiple clients reconnecting at the same time

### 2. Explicit Socket Destruction

- On write errors, the socket is now **explicitly closed** with a proper FIN signal
- Prevents "zombie connections" that keep the port occupied

### 3. Adaptive Drain Freeze

- When disconnected, the app **stops trying to drain the buffer** aggressively
- **Clears stale audio** so you get fresh sound when reconnecting
- Prevents CPU spinning at 100% during network outages

## 📊 Before vs After

| Scenario          | v1.8.9                  | v1.9.0                  |
| ----------------- | ----------------------- | ----------------------- |
| Network drop      | 10 connections in 2 min | Max 1 per 2+ seconds    |
| Reconnect timing  | 1s initial delay        | 2s + jitter             |
| Socket on error   | Silent drop             | Proper FIN + 100ms wait |
| CPU during outage | 100% spin               | Normal pacing           |

## 🚀 Upgrade

Simply replace the old executable with this version on all hotel computers.

> ⚠️ **IMPORTANT**: Deploy to all clients (Lobby, Bar, Restaurant, Pool) to prevent future incidents.

---

**Full Changelog**: [v1.8.9...v1.9.0](../../compare/v1.8.9...v1.9.0)
