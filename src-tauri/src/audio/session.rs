//! Authoritative, monotonically numbered audio sessions.
use serde::Serialize;
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, Emitter};
pub static SESSION: LazyLock<SessionTracker> = LazyLock::new(SessionTracker::default);
pub fn publish(app: &AppHandle, id: u64, state: &str, message: impl Into<String>) {
    if let Some(status) = SESSION.update(id, state, message.into()) {
        let _ = app.emit("stream-state", status);
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct EffectiveConfig {
    pub role: String,
    pub transport: String,
    pub profile: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub device_sample_rate: u32,
    pub device_channels: u16,
    pub ring_ms: u32,
    pub target_ms: u32,
    pub chunk_frames: u32,
    pub capture_buffer_frames: Option<u32>,
    pub adaptive: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SessionStatus {
    pub session_id: u64,
    pub state: String,
    pub message: String,
    pub effective: Option<EffectiveConfig>,
}

#[derive(Default)]
pub struct SessionTracker(Mutex<Option<SessionStatus>>);

impl SessionTracker {
    pub fn snapshot(&self) -> SessionStatus {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or(SessionStatus {
                session_id: 0,
                state: "stopped".into(),
                message: "Ready".into(),
                effective: None,
            })
    }
    pub fn begin(&self) -> SessionStatus {
        let status = SessionStatus {
            session_id: self.snapshot().session_id + 1,
            state: "starting".into(),
            message: "Starting".into(),
            effective: None,
        };
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(status.clone());
        status
    }
    pub fn configure(&self, id: u64, effective: EffectiveConfig) {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(current) = guard.as_mut().filter(|s| s.session_id == id) {
            current.effective = Some(effective);
        }
    }
    pub fn update_runtime(
        &self,
        id: u64,
        rate: u32,
        channels: u16,
        target: u32,
    ) -> Option<SessionStatus> {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let current = guard
            .as_mut()
            .filter(|s| s.session_id == id && !["stopped", "error"].contains(&s.state.as_str()))?;
        let effective = current.effective.as_mut()?;
        if (
            effective.sample_rate,
            effective.channels,
            effective.target_ms,
        ) == (rate, channels, target)
        {
            return None;
        }
        effective.sample_rate = rate;
        effective.channels = channels;
        effective.target_ms = target;
        Some(current.clone())
    }
    pub fn update(&self, id: u64, state: &str, message: String) -> Option<SessionStatus> {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let current = guard.as_mut()?;
        if current.session_id != id
            || (["error", "stopped"].contains(&current.state.as_str())
                && !["error", "stopped"].contains(&state))
        {
            return None;
        }
        current.state = state.into();
        current.message = message;
        Some(current.clone())
    }
}

pub fn interruptible_sleep(running: &std::sync::atomic::AtomicBool, duration: std::time::Duration) {
    let end = std::time::Instant::now() + duration;
    while running.load(std::sync::atomic::Ordering::Relaxed) {
        let Some(left) = end.checked_duration_since(std::time::Instant::now()) else {
            break;
        };
        std::thread::sleep(left.min(std::time::Duration::from_millis(20)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_session_cannot_be_resurrected() {
        let tracker = SessionTracker::default();
        let status = tracker.begin();
        tracker
            .update(status.session_id, "error", "failed".into())
            .unwrap();
        assert!(tracker
            .update(status.session_id, "running", "late audio".into())
            .is_none());
        assert_eq!(tracker.snapshot().state, "error");
    }
    #[test]
    fn old_worker_cannot_change_the_new_session() {
        let tracker = SessionTracker::default();
        let old = tracker.begin();
        let new = tracker.begin();
        assert!(tracker
            .update(old.session_id, "error", "old failure".into())
            .is_none());
        let running = tracker
            .update(new.session_id, "running", "connected".into())
            .unwrap();
        assert_eq!(running.state, "running");
        assert_eq!(tracker.snapshot().message, "connected");
    }
}
