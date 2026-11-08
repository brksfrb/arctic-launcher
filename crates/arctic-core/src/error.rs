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

    /// A Microsoft sign-in that is no longer good (expired, revoked, or refused).
    pub fn is_auth(&self) -> bool {
        matches!(self, Self::Auth(_))
    }

    /// The error in plain words, with what to do about it, for a toast; the raw chain
    /// (paths, URLs, status codes) belongs in the log.
    pub fn plain(&self) -> String {
        match self {
            Self::Http(_) | Self::Checksum(_) => {
                "Couldn't download the game files. Check your connection and press Play again."
                    .into()
            }
            Self::Io { path, source } => {
                let folder = match path.parent() {
                    Some(parent) if !parent.as_os_str().is_empty() => parent,
                    _ => path.as_path(),
                };
                let why = match source.kind() {
                    std::io::ErrorKind::PermissionDenied => "Arctic isn't allowed to write there.",
                    std::io::ErrorKind::NotFound => "The folder is missing.",
                    _ => "Is the disk full or read-only?",
                };
                format!("Couldn't write to {}. {why}", folder.display())
            }
            Self::Json(_) => {
                "A downloaded file was unreadable. Press Play to download it again.".into()
            }
            Self::Auth(_) => "Your Microsoft sign-in has expired. Sign in again to play.".into(),
            Self::NotConfigured(text) | Self::Other(text) => text.clone(),
        }
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
    fn plain_text_names_the_folder_and_keeps_internals_for_the_log() {
        let io = Error::io(
            "/data/libraries/net/fabric.jar",
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        );
        let plain = io.plain();
        assert!(
            plain.starts_with("Couldn't write to /data/libraries/net."),
            "{plain}"
        );
        assert!(plain.contains("isn't allowed"), "{plain}");
        assert!(!plain.contains("denied"), "the OS error stays in the chain");
        let dl = Error::Checksum("https://piston-data.mojang.com/x.jar".into());
        assert!(!dl.plain().contains("https://"));
        assert!(dl.plain().contains("press Play again"));
        assert!(Error::Auth("AADSTS70000: refresh token expired".into()).is_auth());
        assert_eq!(
            Error::Other("no such loader".into()).plain(),
            "no such loader"
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
