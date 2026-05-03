use thiserror::Error;

/// All errors that devpulse can produce.
/// Using thiserror gives us Display + Error implementations for free,
/// and makes every failure case explicit and matchable.
#[derive(Debug, Error)]
pub enum DevpulseError {
    #[error("Failed to spawn process '{binary}': {source}")]
    ProcessSpawn {
        binary: String,
        source: std::io::Error,
    },

    #[error("Process '{binary}' produced invalid UTF-8 output")]
    InvalidUtf8 { binary: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to determine home directory")]
    NoHomeDir,
}

/// Convenience alias — callers write `Result<T>` instead of `Result<T, DevpulseError>`
pub type Result<T> = std::result::Result<T, DevpulseError>;