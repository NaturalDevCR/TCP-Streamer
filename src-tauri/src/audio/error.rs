#![deny(clippy::all)]
#![allow(dead_code)]

use serde::{Serialize, Serializer};
use thiserror::Error;

/// Errors that can occur in audio streaming operations.
/// Implements Serialize so Tauri can send them to the frontend.
#[derive(Error, Debug)]
pub enum AudioError {
    #[error("No audio device found")]
    NoDeviceFound,

    #[error("No supported audio configuration found")]
    NoSupportedConfig,

    #[error("Failed to build audio stream: {0}")]
    StreamBuildError(String),

    #[error("Failed to play audio stream: {0}")]
    StreamPlayError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Internal channel error: {0}")]
    ChannelError(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("IP detection failed: {0}")]
    IpDetectionFailed(String),
}

impl From<std::sync::mpsc::SendError<super::manager::AudioCommand>> for AudioError {
    fn from(e: std::sync::mpsc::SendError<super::manager::AudioCommand>) -> Self {
        AudioError::ChannelError(e.to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for AudioError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        AudioError::ChannelError(e.to_string())
    }
}

impl Serialize for AudioError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipc_error_is_readable_text_not_an_enum_object() {
        let text =
            serde_json::to_value(AudioError::NetworkError("Source is unavailable".into())).unwrap();
        assert_eq!(text.as_str(), Some("Network error: Source is unavailable"));
    }
}
