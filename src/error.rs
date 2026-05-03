use thiserror::Error;

/// All errors devpulse can produce.
/// Variants marked #[allow(dead_code)] are defined for future commands
/// (todos --filter, context --diff) and will be used in upcoming steps.
#[derive(Debug, Error)]
pub enum DevpulseError {
    #[allow(dead_code)]
    #[error("Failed to spawn process '{binary}': {source}")]
    ProcessSpawn {
        binary: String,
        source: std::io::Error,
    },

    #[allow(dead_code)]
    #[error("Process '{binary}' produced invalid UTF-8 output")]
    InvalidUtf8 { binary: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to determine home directory")]
    NoHomeDir,

    #[allow(dead_code)]
    #[error("Not a git repository")]
    NotAGitRepo,

    #[error("Failed to parse session data: {0}")]
    SessionParse(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, DevpulseError>;