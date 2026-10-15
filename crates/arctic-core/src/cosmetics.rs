//! Client for the Arctic cosmetics server (see the `arctic-cosmetics`
//! crate). A player's look (skin, model, cape) is chosen locally and
//! published; every Arctic client then shows it. Nothing is locked: the
//! server only checks who is publishing, so nobody can change someone
//! else's look. Microsoft accounts prove it through Mojang's session server
//! (like a Minecraft server does); offline names are claimed with a key the
//! launcher keeps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

use crate::auth::{Account, AccountKind, now_secs};
use crate::net::agent;
use crate::skins::Variant;
use crate::storage::{DataDirs, load_json, save_json};
use crate::{Error, Result};

pub const DEFAULT_URL: &str = "https://cosmetics.arcticlauncher.com";
const JOIN_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";
const MAX_BYTES: u64 = 2 * 1024 * 1024;
const CREDENTIALS_FILE: &str = "cosmetics.json";
/// Reuse a session token until it's this close to expiring.
const TOKEN_MARGIN_SECS: u64 = 3600;

/// Server to use: `ARCTIC_COSMETICS_URL` or the default.
pub fn base_url() -> String {
    std::env::var(crate::arctic_mod::COSMETICS_URL_ENV)
        .ok()
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .unwrap_or_else(|| DEFAULT_URL.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

/// A cape offered to everyone.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    /// Texture hash.
    pub texture: String,
    /// Animation frames (1: a still cape; older servers don't say).
    #[serde(default = "one_frame")]
    pub frames: u32,
    /// Frames per second when animated (older servers don't say: 8).
    #[serde(default = "default_fps")]
    pub fps: u32,
    /// The still image (texture hash) shown when the wearer turns
    /// "Animate" off; only animated capes have one.
    #[serde(default)]
    pub still: Option<String>,
}

fn one_frame() -> u32 {
    1
}

fn default_fps() -> u32 {
    DEFAULT_CAPE_FPS
}

impl Preset {
    /// A cape that moves (more than one frame).
    pub fn animated(&self) -> bool {
        self.frames > 1
    }

    /// Whether an animated cape can be switched to a still one.
    pub fn has_still(&self) -> bool {
        self.animated() && self.still.is_some()
    }

    /// How long each frame shows.
    pub fn frame_secs(&self) -> f64 {
        1.0 / f64::from(self.fps.clamp(1, MAX_CAPE_FPS))
    }

    /// Whether `hash` is this cape, animated or its still.
    pub fn is_texture(&self, hash: &str) -> bool {
        self.texture == hash || self.still.as_deref() == Some(hash)
    }

    /// What to publish to wear this cape: the animated cape, or (when the
    /// wearer turned "Animate" off and the cape has a still) its still.
    pub fn choice(&self, animate: bool) -> CapeChoice {
        match (&self.still, animate) {
            (Some(still), false) if self.animated() => {
                CapeChoice::Custom(Texture::Hash(still.clone()))
            }
            _ => CapeChoice::Preset(self.id.clone()),
        }
    }
}

/// A published look; textures are hashes (`texture()` fetches them).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Look {
    pub skin: Option<String>,
    #[serde(default)]
    pub model: String,
    pub cape: Option<String>,
    /// Worn 3D cosmetics (catalog ids).
    #[serde(default)]
    pub cosmetics: Vec<String>,
    /// How some of them sit, by id.
    #[serde(default)]
    pub fit: std::collections::BTreeMap<String, Fit>,
}

/// Half pixels a face cosmetic can move up or down (the server keeps it within this).
pub const FIT_UP: i8 = 6;
/// Half pixels a face cosmetic can move forward or back.
pub const FIT_FORWARD: i8 = 4;

/// How a cosmetic sits: moved in half pixels (up; forward, toward the face), or worn on the other
/// side, mirrored (a shoulder pet).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct Fit {
    #[serde(default)]
    pub up: i8,
    #[serde(default)]
    pub forward: i8,
    #[serde(default)]
    pub mirror: bool,
}

impl Fit {
    pub fn is_default(&self) -> bool {
        *self == Fit::default()
    }
}

impl Look {
    pub fn variant(&self) -> Variant {
        if self.model == "slim" {
            Variant::Slim
        } else {
            Variant::Classic
        }
    }
}

/// A texture to publish: a new image, or one the server already has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Texture {
    Png(Vec<u8>),
    Hash(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapeChoice {
    Preset(String),
    Custom(Texture),
}

/// What to publish. `None` parts are removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewLook {
    pub skin: Option<(Texture, Variant)>,
    pub cape: Option<CapeChoice>,
    /// Cosmetics to wear; `None` leaves the current ones on.
    pub cosmetics: Option<Vec<String>>,
    /// How they sit; `None` leaves the current fits.
    pub fit: Option<std::collections::BTreeMap<String, Fit>>,
}

impl NewLook {
    /// The look as currently published (references stored textures).
    pub fn from_look(look: &Look, presets: &[Preset]) -> Self {
        let skin = look
            .skin
            .clone()
            .map(|h| (Texture::Hash(h), look.variant()));
        let cape =
            look.cape
                .as_deref()
                .map(|hash| match presets.iter().find(|p| p.texture == hash) {
                    Some(p) => CapeChoice::Preset(p.id.clone()),
                    // A still image stays the still it is (never silently animated again).
                    None => CapeChoice::Custom(Texture::Hash(hash.to_owned())),
                });
        Self {
            skin,
            cape,
            cosmetics: None,
            fit: None,
        }
    }
}

fn texture_json(texture: &Texture) -> serde_json::Value {
    match texture {
        Texture::Png(png) => serde_json::json!({ "png": STANDARD.encode(png) }),
        Texture::Hash(hash) => serde_json::json!({ "hash": hash }),
    }
}

pub fn catalog(base: &str) -> Result<Vec<Preset>> {
    get(&format!("{base}/v1/catalog"), None)
}

/// The name the server stores a PNG under (its SHA-1), so an uploaded or
/// library image can be matched to a look without asking the server.
pub fn texture_hash(png: &[u8]) -> String {
    use sha1::Digest;
    hex::encode(sha1::Sha1::digest(png))
}

/// A texture by hash (PNG).
pub fn texture(base: &str, hash: &str) -> Result<Vec<u8>> {
    if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Other("bad texture hash".into()));
    }
    let mut resp = agent()
        .get(&format!("{base}/v1/textures/{hash}.png"))
        .call()?;
    Ok(resp
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()?)
}

pub fn my_look(base: &str, token: &str) -> Result<Look> {
    get(&format!("{base}/v1/look"), Some(token))
}

/// Replace the published look.
pub fn set_look(base: &str, token: &str, look: &NewLook) -> Result<Look> {
    let skin = look.skin.as_ref().map(|(texture, variant)| {
        let mut part = texture_json(texture);
        part["model"] = variant.api_name().into();
        part
    });
    let cape = look.cape.as_ref().map(|c| match c {
        CapeChoice::Preset(id) => serde_json::json!({ "preset": id }),
        CapeChoice::Custom(texture) => texture_json(texture),
    });
    let mut resp = agent()
        .put(&format!("{base}/v1/look"))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json({
            let mut body = serde_json::json!({ "skin": skin, "cape": cape });
            if let Some(ids) = &look.cosmetics {
                body["cosmetics"] = serde_json::json!(ids);
            }
            if let Some(fit) = &look.fit {
                body["fit"] = serde_json::json!(fit);
            }
            body
        })?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Other(server_error(&mut resp, status)));
    }
    Ok(resp.body_mut().with_config().limit(MAX_BYTES).read_json()?)
}

pub(crate) fn server_error(resp: &mut ureq::http::Response<ureq::Body>, status: u16) -> String {
    #[derive(Deserialize)]
    struct Body {
        error: String,
    }
    let detail = resp
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .read_json::<Body>()
        .map(|b| b.error)
        .unwrap_or_default();
    match (status, detail.is_empty()) {
        (_, false) => format!("Arctic: {detail}"),
        (429, true) => "Too many changes at once. Wait a minute and try again.".into(),
        _ => format!("The Arctic server returned {status}."),
    }
}

// ---- Gallery ------------------------------------------------------------

/// A skin shared to the community gallery.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GalleryItem {
    pub id: String,
    pub name: String,
    /// Texture hash.
    pub texture: String,
    pub model: String,
    #[serde(default)]
    pub category: String,
    pub author: String,
    #[serde(default)]
    pub downloads: i64,
}

impl GalleryItem {
    pub fn variant(&self) -> Variant {
        if self.model == "slim" {
            Variant::Slim
        } else {
            Variant::Classic
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct GalleryPage {
    pub items: Vec<GalleryItem>,
    pub total: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GallerySort {
    #[default]
    Popular,
    New,
}

/// Skins asked for per request (the server allows 48).
pub const GALLERY_PAGE: usize = 48;

/// Browse the gallery (`query` matches names and authors); `offset` pages on.
pub fn gallery(base: &str, sort: GallerySort, query: &str, offset: usize) -> Result<GalleryPage> {
    gallery_in_collection(base, sort, query, "", offset)
}

/// Browse one curated skin collection; an empty category includes everything.
pub fn gallery_in_collection(
    base: &str,
    sort: GallerySort,
    query: &str,
    category: &str,
    offset: usize,
) -> Result<GalleryPage> {
    let sort = match sort {
        GallerySort::Popular => "popular",
        GallerySort::New => "new",
    };
    // Percent-encode the UTF-8 bytes of everything but letters and digits.
    let q: String = query
        .trim()
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    get(
        &format!(
            "{base}/v1/gallery?sort={sort}&q={q}&category={category}&offset={offset}&limit={GALLERY_PAGE}"
        ),
        None,
    )
}

/// Count a use of a gallery skin and get its texture hash.
pub fn gallery_use(base: &str, id: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Used {
        texture: String,
    }
    let used: Used = post(
        &format!("{base}/v1/gallery/{}/use", url_segment(id)),
        &serde_json::json!({}),
    )?;
    Ok(used.texture)
}

/// Share a skin to the gallery.
pub fn gallery_share(
    base: &str,
    token: &str,
    png: &[u8],
    variant: Variant,
    name: &str,
) -> Result<()> {
    let mut resp = agent()
        .post(&format!("{base}/v1/gallery"))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(serde_json::json!({
            "png": STANDARD.encode(png),
            "model": variant.api_name(),
            "name": name,
        }))?;
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(Error::Other(server_error(&mut resp, status)))
    }
}

/// Send a crash report the player agreed to share.
pub fn send_crash_report(base: &str, report: &crate::crash::Report) -> Result<()> {
    let mut resp = agent()
        .post(&format!("{base}/v1/crash"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(serde_json::json!({
            "launcher": report.launcher,
            "game": report.game,
            "loader": report.loader,
            "os": report.os,
            "title": report.title,
            "log": report.log,
        }))?;
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(Error::Other(server_error(&mut resp, status)))
    }
}

/// An idea or bug report typed into the launcher.
pub struct Suggestion {
    /// `idea` or `bug`.
    pub kind: String,
    pub text: String,
    /// How to reach the sender, if they said.
    pub contact: String,
    /// Logs the sender chose to attach (already cleaned), or empty.
    pub log: String,
}

/// Send a suggestion the player wrote.
pub fn send_suggestion(base: &str, suggestion: &Suggestion) -> Result<()> {
    let mut resp = agent()
        .post(&format!("{base}/v1/suggest"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(serde_json::json!({
            "kind": suggestion.kind,
            "text": suggestion.text,
            "contact": suggestion.contact,
            "launcher": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
            "log": suggestion.log,
        }))?;
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(Error::Other(server_error(&mut resp, status)))
    }
}

/// Report a gallery skin (hidden after several reports).
pub fn gallery_report(base: &str, token: &str, id: &str) -> Result<()> {
    let mut resp = agent()
        .post(&format!("{base}/v1/gallery/{}/report", url_segment(id)))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_empty()?;
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(Error::Other(server_error(&mut resp, status)))
    }
}

/// Gallery ids are hex, or `arctic-<hex>` for the built-in skins; keeping
/// only letters, digits and dashes means an id can't reach another path.
fn url_segment(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

// ---- Sign-in ------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Credential {
    /// Offline accounts: the key that claims the name on the server.
    #[serde(default, with = "crate::secret::on_disk_opt")]
    key: Option<String>,
    #[serde(default, with = "crate::secret::on_disk_opt")]
    token: Option<String>,
    #[serde(default)]
    expires: u64,
    /// The server that issued `token` (tokens only work there).
    #[serde(default)]
    server: String,
}

/// Per-profile store of cosmetics keys and tokens (`cosmetics.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Credentials {
    accounts: HashMap<String, Credential>,
}

fn credentials_path(dirs: &DataDirs) -> PathBuf {
    dirs.profile_root().join(CREDENTIALS_FILE)
}

/// A session token for `account`, reusing a stored one while it's valid.
pub fn token_for(dirs: &DataDirs, base: &str, account: &Account) -> Result<String> {
    let path = credentials_path(dirs);
    let mut creds: Credentials = load_json(&path)?.unwrap_or_default();
    // Files from before encryption: save them encrypted right away.
    if std::fs::read_to_string(&path).is_ok_and(|t| crate::secret::has_plain(&t, &["key", "token"]))
    {
        save_json(&path, &creds)?;
    }
    let entry = creds.accounts.entry(account.uuid.clone()).or_default();
    if let Some(token) = &entry.token
        && entry.expires > now_secs() + TOKEN_MARGIN_SECS
        && entry.server == base
    {
        return Ok(token.clone());
    }
    log::info!(
        "Arctic sign-in for {} ({}) at {base}",
        account.username,
        if account.is_microsoft() {
            "Microsoft"
        } else {
            "offline"
        }
    );
    let session = match &account.kind {
        AccountKind::Microsoft(ms) => sign_in_microsoft(
            base,
            &ms.access_token.reveal(),
            &account.uuid,
            &account.username,
        )
        .inspect_err(|e| log::warn!("Arctic sign-in for {} failed: {e}", account.username))?,
        #[cfg(any(feature = "offline-accounts", test))]
        AccountKind::Offline => {
            let key = usable_key(entry);
            // Save the key before using it, so a claimed name is never lost.
            save_json(&path, &creds)?;
            let session = sign_in_offline(base, &account.username, &key).inspect_err(|e| {
                log::warn!("Arctic sign-in for {} failed: {e}", account.username)
            })?;
            creds = load_json(&path)?.unwrap_or_default();
            session
        }
    };
    log::info!("Arctic sign-in for {} worked", account.username);
    let entry = creds.accounts.entry(account.uuid.clone()).or_default();
    entry.token = Some(session.token.clone());
    entry.expires = session.expires;
    entry.server = base.to_owned();
    save_json(&path, &creds)?;
    Ok(session.token)
}

/// The offline key of an account: the stored one, or a new one when there's none or it can't be
/// used. A stored key that can't be decrypted (the file came from another PC or user, or the
/// system's machine id changed) reads as empty, and the server refuses an empty key as
/// "invalid name or key" for good; a new key at least gets a clear answer (the name is claimed:
/// use the recovery code) instead.
fn usable_key(entry: &mut Credential) -> String {
    let fine = |k: &str| (32..=128).contains(&k.len()) && k.bytes().all(|b| b.is_ascii_hexdigit());
    if !entry.key.as_deref().is_some_and(fine) {
        log::warn!(
            "the stored offline key can't be used (missing, empty or unreadable): making a new one"
        );
        entry.key = Some(
            uuid::Uuid::new_v4().simple().to_string() + &uuid::Uuid::new_v4().simple().to_string(),
        );
        // Whatever token went with the old key is no good either.
        entry.token = None;
    }
    entry.key.clone().unwrap_or_default()
}

/// An offline account on a new PC: the profile's recovery code moves its
/// name to a fresh key made here, and signs it in.
pub fn recover_offline(dirs: &DataDirs, base: &str, account: &Account, code: &str) -> Result<()> {
    if account.is_microsoft() {
        return Err(Error::Other(
            "Microsoft accounts don't need recovering".into(),
        ));
    }
    let key =
        uuid::Uuid::new_v4().simple().to_string() + &uuid::Uuid::new_v4().simple().to_string();
    let mut resp = agent()
        .post(&format!("{base}/v1/profile/recover"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(
            serde_json::json!({ "code": code.trim(), "name": account.username, "key": key }),
        )?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Other(server_error(&mut resp, status)));
    }
    let session: Session = resp.body_mut().with_config().limit(MAX_BYTES).read_json()?;
    let path = credentials_path(dirs);
    let mut creds: Credentials = load_json(&path)?.unwrap_or_default();
    let entry = creds.accounts.entry(account.uuid.clone()).or_default();
    entry.key = Some(key);
    entry.token = Some(session.token);
    entry.expires = session.expires;
    entry.server = base.to_owned();
    save_json(&path, &creds)
}

#[derive(Deserialize)]
struct Session {
    token: String,
    expires: u64,
}

fn sign_in_microsoft(base: &str, access_token: &str, uuid: &str, name: &str) -> Result<Session> {
    #[derive(Deserialize)]
    struct Challenge {
        server_id: String,
    }
    let challenge: Challenge = post(&format!("{base}/v1/auth/challenge"), &serde_json::json!({}))?;
    let joined = agent()
        .post(JOIN_URL)
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(serde_json::json!({
            "accessToken": access_token,
            "selectedProfile": uuid.replace('-', ""),
            "serverId": challenge.server_id,
        }))?;
    if !joined.status().is_success() {
        return Err(Error::Auth(format!(
            "Mojang refused the session ({})",
            joined.status().as_u16()
        )));
    }
    post(
        &format!("{base}/v1/auth/verify"),
        &serde_json::json!({ "name": name, "server_id": challenge.server_id }),
    )
}

#[cfg(any(feature = "offline-accounts", test))]
fn sign_in_offline(base: &str, name: &str, key: &str) -> Result<Session> {
    let mut resp = agent()
        .post(&format!("{base}/v1/auth/offline"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(serde_json::json!({ "name": name, "key": key }))?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::Other(server_error(&mut resp, status)));
    }
    Ok(resp.body_mut().with_config().limit(MAX_BYTES).read_json()?)
}

/// What the Arctic mod gets from the launcher for one game session.
pub struct ModSession<'a> {
    pub base: &'a str,
    /// Cosmetics session, so looks picked in game publish as this player.
    pub token: Option<&'a str>,
    pub style: crate::settings::ClientStyle,
    pub fancy: bool,
    pub style_set: u64,
    pub proxy: &'a crate::proxy::ProxySettings,
    /// In-game account switching, while the launcher runs.
    pub bridge: Option<&'a crate::bridge::BridgeInfo>,
    /// The game may tell the Arctic server (for friends) which server it's on.
    pub share_server: bool,
    /// A duel to start from the title screen (see `QuickPlay::Duel`).
    pub duel: Option<serde_json::Value>,
    /// A replay to watch from the title screen (see `QuickPlay::Replay`).
    pub replay: Option<serde_json::Value>,
}

/// Write the session file for the Arctic mod in `game_dir`.
pub fn write_mod_session(game_dir: &Path, session: &ModSession) -> Result<()> {
    let ModSession {
        base,
        token,
        style,
        fancy,
        style_set,
        proxy,
        bridge,
        share_server,
        duel,
        replay,
    } = session;
    let path = game_dir.join("config").join("arctic-session.json");
    save_json(
        &path,
        &serde_json::json!({
            "url": base,
            "token": token,
            "style": style.id(),
            "fancy": fancy,
            "style_set": style_set,
            // Server connections through the proxy (the Arctic Client does
            // that part; Minecraft only proxies its own web services).
            "proxy": {
                "enabled": proxy.active().is_some(),
                "host": proxy.bare_host(),
                "port": proxy.port,
                "username": proxy.username,
                "password": proxy.password,
                "set": proxy.set,
            },
            "bridge": bridge,
            "share_server": share_server,
            "duel": duel,
            "replay": replay,
        }),
    )
}

fn get<T: serde::de::DeserializeOwned>(url: &str, token: Option<&str>) -> Result<T> {
    let mut req = agent().get(url);
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    let mut resp = req.call()?;
    Ok(resp.body_mut().with_config().limit(MAX_BYTES).read_json()?)
}

fn post<T: serde::de::DeserializeOwned>(url: &str, body: &serde_json::Value) -> Result<T> {
    let mut resp = agent().post(url).send_json(body)?;
    Ok(resp.body_mut().with_config().limit(MAX_BYTES).read_json()?)
}

/// Animated capes stack up to this many 2:1 frames vertically...
pub const MAX_CAPE_FRAMES: u32 = 32;
/// ...in a strip at most this tall (as the server allows).
pub const MAX_CAPE_STRIP_HEIGHT: u32 = 12288;
/// Playback speed of capes that don't say (8 frames a second).
pub const DEFAULT_CAPE_FPS: u32 = 8;
/// Fastest playback a cape may ask for.
pub const MAX_CAPE_FPS: u32 = 30;
/// How long each frame of a cape shows at the default speed.
pub const CAPE_FRAME_SECS: f64 = 1.0 / DEFAULT_CAPE_FPS as f64;

static REDUCE_CAPE_MOTION: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// The viewer's "reduce motion" choice: animated capes stand still on the
/// first frame (everywhere in the launcher, for every cape shown).
pub fn set_reduce_cape_motion(on: bool) {
    REDUCE_CAPE_MOTION.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn reduce_cape_motion() -> bool {
    REDUCE_CAPE_MOTION.load(std::sync::atomic::Ordering::Relaxed)
}

/// Width and height from a PNG's header, without decoding the pixels.
pub fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    let reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .ok()?;
    Some((reader.info().width, reader.info().height))
}

/// Widest cape accepted (HD capes, as the server allows).
pub const MAX_CAPE_WIDTH: u32 = 1024;

/// Whether a PNG is a cape the server would accept, judged from its header
/// before anything is decoded.
pub fn is_cape_png(png: &[u8]) -> bool {
    png_size(png).is_some_and(|(w, h)| {
        (64..=MAX_CAPE_WIDTH).contains(&w) && w.is_power_of_two() && cape_frames(w, h).is_some()
    })
}

/// Frames in a cape image of `width`×`height`: 1 for a plain cape, more
/// for animated ones, `None` if it isn't a cape layout.
pub fn cape_frames(width: u32, height: u32) -> Option<u32> {
    let frame = width / 2;
    if frame == 0 || !height.is_multiple_of(frame) {
        return None;
    }
    Some(height / frame)
        .filter(|n| (1..=MAX_CAPE_FRAMES).contains(n))
        .filter(|_| height <= MAX_CAPE_STRIP_HEIGHT)
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_unreadable_or_missing_offline_key_is_replaced_never_sent_empty() {
        for stored in [None, Some(String::new()), Some("not hex".to_owned())] {
            let mut entry = Credential {
                key: stored,
                token: Some("old".into()),
                ..Credential::default()
            };
            let key = usable_key(&mut entry);
            assert_eq!(key.len(), 64);
            assert!(key.bytes().all(|b| b.is_ascii_hexdigit()));
            assert_eq!(entry.token, None, "the old key's token goes too");
        }
        let good = "ab".repeat(32);
        let mut entry = Credential {
            key: Some(good.clone()),
            token: Some("t".into()),
            ..Credential::default()
        };
        assert_eq!(usable_key(&mut entry), good);
        assert_eq!(
            entry.token.as_deref(),
            Some("t"),
            "a good key is left alone"
        );
    }

    use super::*;

    #[test]
    fn gallery_ids_stay_whole_but_cannot_escape_the_path() {
        assert_eq!(
            url_segment("arctic-eb46302ea78bd2a1"),
            "arctic-eb46302ea78bd2a1"
        );
        assert_eq!(url_segment("003f9230304949ff"), "003f9230304949ff");
        assert_eq!(url_segment("../admin?x=1"), "adminx1");
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        let mut writer = enc.write_header().unwrap();
        writer
            .write_image_data(&vec![255u8; (w * h * 4) as usize])
            .unwrap();
        drop(writer);
        out
    }

    #[test]
    fn texture_hash_is_the_sha1_the_server_names_it_by() {
        // SHA-1 of the empty input.
        assert_eq!(
            texture_hash(b""),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        assert_eq!(texture_hash(&png(64, 64)).len(), 40);
    }

    #[test]
    fn cape_pngs_are_judged_by_their_header() {
        assert!(is_cape_png(&png(64, 32)));
        assert!(is_cape_png(&png(128, 64 * 4)));
        assert!(is_cape_png(&png(1024, 512)));
        assert!(!is_cape_png(&png(2048, 1024)));
        assert!(!is_cape_png(&png(96, 48)));
        assert!(!is_cape_png(b"not a png"));
        // A small file claiming a huge size is refused before decoding.
        let mut huge = png(64, 32);
        huge[16..20].copy_from_slice(&65_536u32.to_be_bytes());
        huge[20..24].copy_from_slice(&32_768u32.to_be_bytes());
        assert!(!is_cape_png(&huge));
    }

    #[test]
    fn counts_cape_frames() {
        assert_eq!(cape_frames(64, 32), Some(1));
        assert_eq!(cape_frames(128, 64 * 6), Some(6));
        assert_eq!(cape_frames(64, 32 * 9), Some(9));
        assert_eq!(cape_frames(64, 32 * 33), None);
        // The strip may be at most 12288 tall: 24 frames of 1024×512, not 25.
        assert_eq!(cape_frames(1024, 512 * 24), Some(24));
        assert_eq!(cape_frames(1024, 512 * 25), None);
        assert_eq!(cape_frames(512, 256 * 32), Some(32));
        assert_eq!(cape_frames(64, 48), None);
        assert_eq!(cape_frames(0, 0), None);
    }

    fn preset(frames: u32, still: Option<&str>) -> Preset {
        Preset {
            id: "wolf".into(),
            name: "Wolf".into(),
            texture: "aa".into(),
            frames,
            fps: 12,
            still: still.map(str::to_owned),
        }
    }

    #[test]
    fn animate_off_wears_the_still_and_keeps_it_across_republishes() {
        let wolf = preset(16, Some("bb"));
        assert_eq!(wolf.choice(true), CapeChoice::Preset("wolf".into()));
        assert_eq!(
            wolf.choice(false),
            CapeChoice::Custom(Texture::Hash("bb".into()))
        );
        assert!((wolf.frame_secs() - 1.0 / 12.0).abs() < 1e-9);
        assert!(wolf.is_texture("aa") && wolf.is_texture("bb") && !wolf.is_texture("cc"));
        // A cape that doesn't move has no still to switch to.
        assert_eq!(
            preset(1, None).choice(false),
            CapeChoice::Preset("wolf".into())
        );
        // Re-publishing a look that wears the still keeps the still.
        let look = Look {
            cape: Some("bb".into()),
            ..Look::default()
        };
        let again = NewLook::from_look(&look, &[wolf]);
        assert_eq!(
            again.cape,
            Some(CapeChoice::Custom(Texture::Hash("bb".into())))
        );
    }

    #[test]
    fn older_servers_have_no_fps_or_still() {
        let p: Preset =
            serde_json::from_str(r#"{"id":"a","name":"A","texture":"t","frames":6}"#).unwrap();
        assert_eq!((p.fps, p.still.clone()), (8, None));
        assert!(p.animated() && !p.has_still());
    }

    #[test]
    fn parses_looks() {
        let look: Look =
            serde_json::from_str(r#"{"skin":"ab","model":"slim","cape":null}"#).unwrap();
        assert_eq!(look.variant(), Variant::Slim);
        assert_eq!(look.cape, None);
        let empty: Look = serde_json::from_str(r#"{"skin":null,"cape":null}"#).unwrap();
        assert_eq!(empty.variant(), Variant::Classic);
    }

    #[test]
    fn rejects_odd_hashes() {
        assert!(texture("http://127.0.0.1:1", "../x").is_err());
    }
}
