use std::path::PathBuf;

/// Core error type. Messages are written to be shown to the user as-is.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error: {0}")]
    Http(String),

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid data: {0}")]
    Json(#[from] serde_json::Error),

    #[error("checksum mismatch for {0}")]
    Checksum(String),

    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("{0}")]
    NotConfigured(String),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl From<ureq::Error> for Error {
    fn from(e: ureq::Error) -> Self {
        Self::Http(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Extension to attach a path to `std::io::Result`.
pub trait IoContext<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T> {
        self.map_err(|e| Error::io(path, e))
    }
}
