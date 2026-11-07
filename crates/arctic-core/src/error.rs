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

    /// Display text plus every `source()` that it does not already end with.
    pub fn chain(&self) -> String {
        error_chain(self)
    }
}

/// How many causes to follow. A cycle in `source()` cannot loop forever.
const MAX_CAUSES: usize = 8;

/// The error and its sources, joined with `": "`. A cause already printed at the
/// end of the parent (thiserror's `Io` and `Json` variants do this) is not repeated.
fn error_chain(err: &dyn std::error::Error) -> String {
    let mut out = err.to_string();
    let mut cause = std::error::Error::source(err);
    for _ in 0..MAX_CAUSES {
        let Some(next) = cause else {
            break;
        };
        let text = next.to_string();
        if !text.is_empty() && !already_shown(&out, &text) {
            out.push_str(": ");
            out.push_str(&text);
        }
        cause = std::error::Error::source(next);
    }
    out
}

fn already_shown(message: &str, cause: &str) -> bool {
    message == cause || message.ends_with(&format!(": {cause}"))
}

impl From<ureq::Error> for Error {
    fn from(e: ureq::Error) -> Self {
        Self::Http(error_chain(&e))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Leaf;

    impl std::fmt::Display for Leaf {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("timeout: receive response")
        }
    }

    impl std::error::Error for Leaf {}

    #[derive(Debug)]
    struct Wrap;

    impl std::fmt::Display for Wrap {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // Display stops at this layer; the cause is only on `source()`.
            f.write_str("network error")
        }
    }

    impl std::error::Error for Wrap {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            static LEAF: Leaf = Leaf;
            Some(&LEAF)
        }
    }

    #[test]
    fn chain_appends_sources_the_display_leaves_out() {
        assert_eq!(
            error_chain(&Wrap),
            "network error: timeout: receive response"
        );
    }

    #[test]
    fn chain_does_not_repeat_a_cause_already_in_the_message() {
        let err = Error::io("libraries/fabric.jar", std::io::Error::other("disk full"));
        let chain = err.chain();
        assert!(chain.contains("libraries/fabric.jar"), "{chain}");
        assert!(chain.contains("disk full"), "{chain}");
        assert_eq!(chain.matches("disk full").count(), 1, "{chain}");
    }
}
