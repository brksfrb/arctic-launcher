//! Sharing and backups: an instance (as a mod list), a HUD layout, a
//! crosshair, all Arctic Client settings, or a whole profile, as a small
//! JSON bundle. A bundle travels as a `.json` file, one line of text
//! (`arctic1.…`) or a short code stored on the Arctic server.
//!
//! Account data never goes in a bundle, and neither do Java paths or JVM
//! flags (they could run programs on the other PC). Every bundle read is
//! checked field by field before anything is written.

pub mod client;
pub mod codes;
mod instance;
mod profile;
#[cfg(test)]
mod tests;

use std::io::{Read, Write};
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};

pub use client::{ClientPart, Part};
pub use instance::{Exported, InstancePack, SharedMod};
pub use profile::{ProfilePack, ProfileReport};

use crate::{Error, Result};

/// Bundle format version.
pub const FORMAT: u64 = 1;
/// Largest bundle read (matches the server's limit).
pub const MAX_JSON: usize = 256 * 1024;
const TEXT_PREFIX: &str = "arctic1.";

#[derive(Debug, Clone, PartialEq)]
pub enum Bundle {
    Instance(InstancePack),
    /// A HUD layout, a crosshair, or all client settings.
    Client(ClientPart),
    Profile(ProfilePack),
}

/// What someone pasted.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// A short code; fetch it with [`codes::fetch`].
    Code(String),
    Bundle(Bundle),
}

impl Bundle {
    pub fn kind(&self) -> &'static str {
        match self {
            Bundle::Instance(_) => "instance",
            Bundle::Client(c) => match c.part {
                Part::Hud => "hud",
                Part::Crosshair => "crosshair",
                Part::All => "client",
            },
            Bundle::Profile(_) => "profile",
        }
    }

    pub fn to_value(&self) -> Value {
        let data = match self {
            Bundle::Instance(i) => serde_json::to_value(i).unwrap_or(Value::Null),
            Bundle::Client(c) => Value::Object(c.values.clone()),
            Bundle::Profile(p) => serde_json::to_value(p).unwrap_or(Value::Null),
        };
        json!({ "arctic_share": FORMAT, "kind": self.kind(), "data": data })
    }

    /// Parse and check a bundle.
    pub fn from_value(value: &Value) -> Result<Self> {
        let format = value
            .get("arctic_share")
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::Other("that isn't something Arctic shared".into()))?;
        if format > FORMAT {
            return Err(Error::Other(
                "this was shared from a newer Arctic; update the launcher to use it".into(),
            ));
        }
        let data = value.get("data").cloned().unwrap_or(Value::Null);
        let parse_err = |e: serde_json::Error| Error::Other(format!("the share is damaged: {e}"));
        match value.get("kind").and_then(Value::as_str) {
            Some("instance") => Ok(Bundle::Instance(
                serde_json::from_value::<InstancePack>(data)
                    .map_err(parse_err)?
                    .check()?,
            )),
            Some("profile") => Ok(Bundle::Profile(
                serde_json::from_value::<ProfilePack>(data)
                    .map_err(parse_err)?
                    .check()?,
            )),
            Some("hud") => Ok(Bundle::Client(ClientPart::check(Part::Hud, &data)?)),
            Some("crosshair") => Ok(Bundle::Client(ClientPart::check(Part::Crosshair, &data)?)),
            Some("client") => Ok(Bundle::Client(ClientPart::check(Part::All, &data)?)),
            _ => Err(Error::Other(
                "this share holds something this Arctic doesn't know; try updating".into(),
            )),
        }
    }

    /// One line of text: `arctic1.` + base64 of the deflated JSON.
    pub fn to_text(&self) -> String {
        let json = self.to_value().to_string();
        let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        let packed = enc
            .write_all(json.as_bytes())
            .and_then(|()| enc.finish())
            .unwrap_or_default();
        format!("{TEXT_PREFIX}{}", URL_SAFE_NO_PAD.encode(packed))
    }

    /// Pretty JSON for a `.json` file.
    pub fn to_file_text(&self) -> String {
        serde_json::to_string_pretty(&self.to_value()).unwrap_or_default()
    }

    /// One line saying what's inside.
    pub fn summary(&self) -> String {
        match self {
            Bundle::Instance(i) => format!("Instance {}", i.summary()),
            Bundle::Profile(p) => p.summary(),
            Bundle::Client(c) => match (c.part, c.widgets_on()) {
                (Part::Hud, Some(n)) => format!("A HUD layout ({n} widgets on)"),
                (Part::Hud, None) => "A HUD layout".into(),
                (Part::Crosshair, _) => "A crosshair".into(),
                (Part::All, _) => format!(
                    "Arctic Client settings ({} of them: HUD, crosshair, features, keys)",
                    c.values.len()
                ),
            },
        }
    }
}

/// Make sense of pasted text: a code, `arctic1.` text or bundle JSON.
pub fn read(input: &str) -> Result<Input> {
    let input = input.trim();
    if input.len() > MAX_JSON * 2 {
        return Err(Error::Other("that is too long to be a share".into()));
    }
    if let Some(packed) = input.strip_prefix(TEXT_PREFIX) {
        return from_text(packed).map(Input::Bundle);
    }
    if input.starts_with('{') {
        let value: Value =
            serde_json::from_str(input).map_err(|_| Error::Other("that JSON is damaged".into()))?;
        return Bundle::from_value(&value).map(Input::Bundle);
    }
    codes::clean(input).map(Input::Code).ok_or_else(|| {
        Error::Other("paste a share code (like abcd-efgh), share text or a .json file".into())
    })
}

/// A `.json` (or text) file someone exported.
pub fn read_file(path: &Path) -> Result<Bundle> {
    let size = std::fs::metadata(path)
        .map_err(|e| Error::io(path, e))?
        .len();
    if size > (MAX_JSON * 2) as u64 {
        return Err(Error::Other("that file is too big to be a share".into()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
    match read(&text)? {
        Input::Bundle(b) => Ok(b),
        Input::Code(code) => Err(Error::Other(format!(
            "that file only holds the code {}; paste it instead",
            codes::pretty(&code)
        ))),
    }
}

fn from_text(packed: &str) -> Result<Bundle> {
    let damaged = || Error::Other("that share text is damaged or cut off".into());
    let packed: String = packed.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = URL_SAFE_NO_PAD.decode(packed).map_err(|_| damaged())?;
    let mut json = Vec::new();
    flate2::read::DeflateDecoder::new(bytes.as_slice())
        .take(MAX_JSON as u64 + 1)
        .read_to_end(&mut json)
        .map_err(|_| damaged())?;
    if json.len() > MAX_JSON {
        return Err(Error::Other("that share is too big".into()));
    }
    let value: Value = serde_json::from_slice(&json).map_err(|_| damaged())?;
    Bundle::from_value(&value)
}
