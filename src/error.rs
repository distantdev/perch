use thiserror::Error;

#[derive(Debug, Error)]
pub enum PerchError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("scan failed: {0}")]
    Scan(String),

    #[error("process control failed: {0}")]
    ProcessControl(String),

    #[error("permission denied for PID {pid}: {reason}")]
    PermissionDenied { pid: u32, reason: String },

    #[error("no listener found on port {0}")]
    PortNotFound(u16),

    #[error("process {0} not found")]
    ProcessNotFound(u32),

    #[error("invalid target: {0}")]
    InvalidTarget(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, PerchError>;
