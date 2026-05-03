use thiserror::Error;

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

    #[error("Not a git repository")]
    NotAGitRepo,

    #[error("Failed to parse session data: {0}")]
    SessionParse(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, DevpulseError>;