//! Portable appearance commands for an already-loaded native client.
//! Completion acknowledges Java archive/renderer state, never native parity.
use super::Edition;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum AppearanceControl {
    HistoryBegin {
        path: PathBuf,
        #[serde(rename = "timeMs")]
        time_ms: u32,
        owner: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        anchor: Option<RecordingAnchor>,
    },
    HistoryFinish {
        owner: String,
    },
    HistoryCancel {
        owner: String,
    },
    Save {
        path: PathBuf,
    },
    Load {
        path: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        owner: Option<String>,
    },
    Time {
        #[serde(rename = "timeMs")]
        time_ms: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        owner: Option<String>,
    },
    Clear {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        owner: Option<String>,
    },
    State,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordingAnchor {
    pub recording_id: String,
    pub sequence: u64,
    pub request_id: String,
    pub marker: String,
    pub generation: u64,
    pub replay_sequence: u64,
}
impl AppearanceControl {
    #[cfg(windows)]
    fn validate(&mut self) -> Result<()> {
        use crate::error::IoContext;
        let owner = match self {
            Self::Load { owner, .. } | Self::Time { owner, .. } | Self::Clear { owner } => {
                owner.as_deref()
            }
            Self::HistoryBegin { owner, .. }
            | Self::HistoryFinish { owner }
            | Self::HistoryCancel { owner } => Some(owner.as_str()),
            _ => None,
        };
        if owner.is_some_and(|owner| {
            owner.is_empty()
                || owner.len() > 64
                || !owner
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        }) {
            return Err(Error::Other(
                "appearance owner needs 1–64 letters, digits, underscores or hyphens".into(),
            ));
        }
        match self {
            Self::HistoryBegin {
                anchor: Some(anchor),
                ..
            } if uuid::Uuid::parse_str(&anchor.recording_id)
                .ok()
                .is_none_or(|id| id.to_string() != anchor.recording_id)
                || anchor.request_id.is_empty()
                || anchor.request_id.len() > 64
                || !anchor
                    .request_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                || ![anchor.sequence, anchor.generation, anchor.replay_sequence]
                    .iter()
                    .all(|s| (1..=1_000_000_000).contains(s))
                || anchor.marker.len() != 16
                || !anchor.marker.starts_with("37c1")
                || !anchor
                    .marker
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
            {
                return Err(Error::Other("invalid recording anchor identity".into()));
            }
            Self::Save { path } | Self::Load { path, .. } | Self::HistoryBegin { path, .. } => {
                *path = std::path::absolute(&*path).at(&*path)?;
                if !path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(".arctic-looks.zip"))
                {
                    return Err(Error::Other(
                        "appearance files use the .arctic-looks.zip extension".into(),
                    ));
                }
            }
            Self::Time { time_ms, .. } if *time_ms > 86_400_000 => {
                return Err(Error::Other("appearance time exceeds 24 hours".into()));
            }
            _ => {}
        }
        match self {
            Self::HistoryBegin { time_ms, .. } if *time_ms > 86_400_000 => Err(Error::Other(
                "appearance history origin exceeds 24 hours".into(),
            )),
            Self::HistoryBegin { path, .. }
                if path.exists() || !path.parent().is_some_and(|p| p.is_dir()) =>
            {
                Err(Error::Other(
                    "appearance history output must be new, in an existing directory".into(),
                ))
            }
            Self::Save { path } if path.exists() || !path.parent().is_some_and(|p| p.is_dir()) => {
                Err(Error::Other(
                    "appearance output must be new, in an existing directory".into(),
                ))
            }
            Self::Load { path, .. } if !path.is_file() => {
                Err(Error::Other("appearance input does not exist".into()))
            }
            _ => Ok(()),
        }
    }
}

/// Worker-only. Never loads a DLL, opens a game, connects a world or signs in.
#[cfg(windows)]
pub fn control(
    edition: Edition,
    bundle: &Path,
    session: &str,
    sequence: u64,
    mut operation: AppearanceControl,
) -> Result<serde_json::Value> {
    use crate::error::IoContext;
    use std::{
        io::{Read, Write},
        time::{Duration, Instant},
    };
    operation.validate()?;
    if session.is_empty()
        || session.len() > 64
        || !session
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        || !(1..=1_000_000_000).contains(&sequence)
    {
        return Err(Error::Other(
            "appearance command needs a native session and increasing sequence".into(),
        ));
    }
    let bundle = super::client::validate_bundle(bundle)?;
    let installed =
        super::find(edition).ok_or_else(|| Error::Other("Bedrock isn't installed".into()))?;
    let pid =
        super::package::process_id(edition.package_family(), &installed.path).ok_or_else(|| {
            Error::Other("appearance control needs the already-running native client".into())
        })?;
    let prefix = format!("native_{pid}_");
    if !session
        .strip_prefix(prefix.as_str())
        .and_then(|e| e.parse::<u64>().ok())
        .is_some_and(|e| e > 0)
        || !std::fs::read_to_string(bundle.join(format!("arctic-bedrock-{pid}.ready")))
            .is_ok_and(|v| v.trim() == "native-preview")
    {
        return Err(Error::Other(
            "appearance command does not identify this loaded process/world".into(),
        ));
    }
    let request_id = format!("appearance-{}", uuid::Uuid::new_v4().simple());
    let mut value = serde_json::to_value(&operation)?;
    value["schema"] = serde_json::json!(1);
    value["session"] = serde_json::json!(session);
    value["sequence"] = serde_json::json!(sequence);
    value["requestId"] = serde_json::json!(request_id);
    let bytes = serde_json::to_vec(&value)?;
    if bytes.len() > 16 * 1024 {
        return Err(Error::Other("appearance command exceeds 16 KiB".into()));
    }
    let request = bundle.join(format!("arctic-bedrock-{pid}.appearance-request.json"));
    let temporary = bundle.join(format!(".{request_id}.tmp"));
    let publish = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .at(&temporary)?;
        file.write_all(&bytes).at(&temporary)?;
        file.sync_all().at(&temporary)?;
        drop(file);
        std::fs::hard_link(&temporary, &request).at(&request)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    publish?;
    let report = bundle.join(format!("arctic-bedrock-{pid}.appearance.json"));
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(30) {
        let mut output = Vec::new();
        if let Ok(file) = std::fs::File::open(&report)
            && file
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age < Duration::from_secs(2))
            && file.take(64 * 1024 + 1).read_to_end(&mut output).is_ok()
            && output.len() <= 64 * 1024
            && let Ok(status) = serde_json::from_slice::<serde_json::Value>(&output)
        {
            let history = status["history"]
                .as_array()
                .into_iter()
                .flat_map(|records| records.iter().rev());
            for receipt in std::iter::once(&status["requestRejected"])
                .chain(std::iter::once(&status["last"]))
                .chain(history)
            {
                if receipt["session"].as_str() != Some(session)
                    || receipt["sequence"].as_u64() != Some(sequence)
                    || receipt["requestId"].as_str() != Some(request_id.as_str())
                {
                    continue;
                }
                if let Some(error) = receipt["error"].as_str() {
                    return Err(Error::Other(error.into()));
                }
                if receipt["stage"].as_str() != Some("completed") || receipt["op"] != value["op"] {
                    continue;
                }
                if status["session"].as_str() != Some(session) {
                    return Err(Error::Other(
                        "native world changed after the appearance operation completed".into(),
                    ));
                }
                if status["contextLive"].as_bool() != Some(true) {
                    continue;
                }
                let state = &receipt["state"];
                if state["session"].as_str() != Some(session) {
                    return Err(Error::Other(
                        "appearance consumer returned a different Java world".into(),
                    ));
                }
                let valid = match &operation {
                    AppearanceControl::HistoryBegin { owner, .. } => {
                        state["history"]["session"].as_str() == Some(session)
                            && state["history"]["owner"].as_str() == Some(owner.as_str())
                            && state["history"]["stage"].as_str() == Some("recording")
                    }
                    AppearanceControl::HistoryFinish { owner } => {
                        state["history"]["session"].as_str() == Some(session)
                            && state["history"]["owner"].as_str() == Some(owner.as_str())
                            && matches!(
                                state["history"]["stage"].as_str(),
                                Some("finishing" | "complete")
                            )
                    }
                    AppearanceControl::HistoryCancel { owner } => {
                        state["history"]["session"].as_str() == Some(session)
                            && state["history"]["owner"].as_str() == Some(owner.as_str())
                            && state["history"]["stage"].as_str() == Some("failed")
                    }
                    AppearanceControl::Load { owner, .. } => {
                        state["loaded"].as_bool() == Some(true)
                            && state["appearance"]["owner"].as_str() == owner.as_deref()
                            && state["appearance"]["manifestHash"]
                                .as_str()
                                .is_some_and(|h| {
                                    h.len() == 64
                                        && h.bytes().all(|b| {
                                            b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                                        })
                                })
                    }
                    AppearanceControl::Time { time_ms, owner } => {
                        state["loaded"].as_bool() == Some(true)
                            && state["appearance"]["owner"].as_str() == owner.as_deref()
                            && state["appearance"]["timeMs"].as_u64() == Some(u64::from(*time_ms))
                    }
                    AppearanceControl::Clear { .. } => state["loaded"].as_bool() == Some(false),
                    _ => true,
                };
                if !valid {
                    return Err(Error::Other(
                        "appearance consumer did not acknowledge the requested state".into(),
                    ));
                }
                return Ok(receipt.clone());
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if std::fs::read(&request).is_ok_and(|value| value == bytes) {
        let _ = std::fs::remove_file(&request);
    }
    Err(Error::Other(format!(
        "appearance acknowledgement timed out; an admitted operation may still be executing; inspect {}",
        report.display()
    )))
}

#[cfg(not(windows))]
pub fn control(
    _: Edition,
    _: &Path,
    _: &str,
    _: u64,
    _: AppearanceControl,
) -> Result<serde_json::Value> {
    Err(Error::Other(
        "native appearance control requires Windows".into(),
    ))
}
