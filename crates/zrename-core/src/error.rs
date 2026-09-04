use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    Io0(#[from] std::io::Error),

    #[error("invalid regex in rule: {0}")]
    Regex(#[from] regex::Error),

    #[error("invalid glob pattern: {0}")]
    Glob(#[from] globset::Error),

    #[error("preset error: {0}")]
    Preset(String),

    #[error("journal error: {0}")]
    Journal(String),

    #[error("csv error: {0}")]
    Csv(String),

    #[error("{0}")]
    Other(String),
}

impl CoreError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
