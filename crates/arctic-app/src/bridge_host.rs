//! The launcher side of the in-game account switcher (see
//! `arctic_core::bridge`): answers the running game with this profile's
//! accounts, refreshing logins through the same task path as launches so
//! the app's account list stays the one source of truth.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use arctic_core::auth::{Account, AccountKind, AccountStore};
use arctic_core::bridge::{self, AccountEntry, BridgeInfo, SessionGrant};
use arctic_core::cosmetics;

use crate::tasks::Tasks;
use crate::{t, tf};
struct Shared {
    accounts: Vec<Account>,
    active: Option<String>,
    tasks: Tasks,
    voice: Option<crate::voice::VoiceHub>,
    /// What the game asked play together to do (the app picks it up).
    together_request: Option<TogetherRequest>,
    /// Play together's state, for the game (set by the app every frame).
    together_status: String,
    /// An account sign-in started from the game, for the game to follow.
    login_status: String,
    /// FFmpeg being fetched for replay videos.
    ffmpeg: FfmpegJob,
    /// Accounts the game may get a session for: the ones games were launched with, and the ones
    /// the player approved in the launcher. Anything else needs a yes first.
    allowed: HashSet<String>,
    /// A switch to an account that is not allowed yet, waiting for the player's answer.
    switch_ask: Option<SwitchAsk>,
    asks_made: u64,
    /// Running games by their launch token: what to tell each to join.
    games: HashMap<String, GameLink>,
}

/// How one launched game reaches the launcher for joins.
struct GameLink {
    run: u64,
    /// A server address waiting for the game to pick it up.
    join: Option<String>,
    /// The last time the game asked (a game without the Arctic Client never does).
    polled: Option<std::time::Instant>,
}

/// The Arctic Client asks about once a second; a game that has not asked for this long has no client
/// (or has not finished starting).
const GAME_ALIVE: std::time::Duration = std::time::Duration::from_secs(5);

/// The game asked to switch to `name`; the launcher asks the player.
struct SwitchAsk {
    id: String,
    name: String,
    answer: Option<bool>,
    asked: std::time::Instant,
    /// Which question this is, so an answer only counts for the one the player saw.
    serial: u64,
}

/// How long a session request waits for the player before telling the game to ask again. Kept
/// under the game's own 10 s read timeout.
const SWITCH_WAIT: std::time::Duration = std::time::Duration::from_secs(8);
/// A question nobody answered this long is dropped, so a game that stopped asking cannot keep
/// the launcher's dialog (or the next request) stuck.
const SWITCH_STALE: std::time::Duration = std::time::Duration::from_secs(60);
const SWITCH_POLL: std::time::Duration = std::time::Duration::from_millis(100);

/// A download of FFmpeg the game asked for.
#[derive(Default)]
struct FfmpegJob {
    running: bool,
    done: u64,
    total: u64,
    error: Option<String>,
}

/// What the game can ask of play together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TogetherRequest {
    Host,
    Join(String),
    Stop,
}

#[derive(serde::Deserialize)]
struct TogetherBody {
    action: String,
    #[serde(default)]
    code: String,
}

pub struct BridgeHost {
    pub info: Option<Arc<BridgeInfo>>,
    shared: Arc<Mutex<Shared>>,
}

struct AppAccounts(Arc<Mutex<Shared>>);

impl AppAccounts {
    /// Lets a session for `account` through when it is offline, a game was launched with it or the
    /// player said yes; otherwise puts the question to the player and waits a moment for the answer. A game
    /// (or any mod in it) can therefore not collect the tokens of the other accounts.
    fn require_approval(&self, account: &Account, tasks: &Tasks) -> arctic_core::Result<()> {
        // An offline account's login is a placeholder: there is nothing in it to steal.
        if !matches!(account.kind, AccountKind::Microsoft(_)) {
            return Ok(());
        }
        let deadline = std::time::Instant::now() + SWITCH_WAIT;
        loop {
            {
                let mut shared = self.0.lock().unwrap_or_else(PoisonError::into_inner);
                if shared.allowed.contains(&account.id) {
                    return Ok(());
                }
                if shared
                    .switch_ask
                    .as_ref()
                    .is_some_and(|ask| ask.asked.elapsed() > SWITCH_STALE)
                {
                    shared.switch_ask = None;
                }
                match shared.switch_ask.as_ref() {
                    // Still waiting for the player.
                    Some(ask) if ask.id == account.id && ask.answer.is_none() => {}
                    Some(ask) if ask.id == account.id => {
                        shared.switch_ask = None;
                        return Err(arctic_core::Error::Other(
                            "the switch was declined in the launcher".into(),
                        ));
                    }
                    Some(ask) if ask.answer.is_none() => {
                        return Err(arctic_core::Error::Other(
                            "the launcher is asking about another switch".into(),
                        ));
                    }
                    // Nothing asked, or an old answer about another account.
                    _ => {
                        log::info!("the game asks to switch to {}", account.username);
                        // The launcher is usually behind the game, where the question would go unseen.
                        crate::window::show();
                        tasks.wake();
                        shared.asks_made += 1;
                        let serial = shared.asks_made;
                        shared.switch_ask = Some(SwitchAsk {
                            serial,
                            id: account.id.clone(),
                            name: account.username.clone(),
                            answer: None,
                            asked: std::time::Instant::now(),
                        });
                    }
                }
            }
            if std::time::Instant::now() >= deadline {
                // The game asks again by itself; the question stays up in the launcher.
                return Err(arctic_core::Error::Other(bridge::WAITING.into()));
            }
            std::thread::sleep(SWITCH_POLL);
        }
    }
}

impl bridge::Accounts for AppAccounts {
    fn list(&self) -> Vec<AccountEntry> {
        let shared = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        shared
            .accounts
            .iter()
            .map(|a| AccountEntry {
                id: a.id.clone(),
                name: a.username.clone(),
                uuid: a.uuid.clone(),
                microsoft: matches!(a.kind, AccountKind::Microsoft(_)),
                active: shared.active.as_deref() == Some(a.id.as_str()),
            })
            .collect()
    }

    fn session(&self, id: &str) -> arctic_core::Result<SessionGrant> {
        let (account, tasks) = {
            let shared = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            let account = shared
                .accounts
                .iter()
                .find(|a| a.id == id)
                .cloned()
                .ok_or_else(|| arctic_core::Error::Other("no such account".into()))?;
            (account, shared.tasks.clone())
        };
        self.require_approval(&account, &tasks)?;
        let fresh = tasks.fresh_account(account)?;
        let arctic_token = cosmetics::token_for(tasks.dirs(), &cosmetics::base_url(), &fresh)
            .inspect_err(|e| log::info!("Arctic session for the switch: {e}"))
            .ok();
        Ok(SessionGrant {
            identity: fresh.identity(),
            arctic_token,
        })
    }

    fn add(&self) -> arctic_core::Result<()> {
        let tasks = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tasks
            .clone();
        tasks.send(crate::tasks::Event::AddAccountFromGame);
        Ok(())
    }

    fn add_offline(&self, name: &str) -> arctic_core::Result<()> {
        // Same rule as the launcher's own "Add offline account".
        arctic_core::auth::offline::validate_username(name.trim())?;
        let tasks = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tasks
            .clone();
        tasks.send(crate::tasks::Event::AddOfflineFromGame(
            name.trim().to_owned(),
        ));
        Ok(())
    }

    fn skins(&self) -> arctic_core::Result<String> {
        let dirs = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tasks
            .dirs()
            .clone();
        arctic_core::skins::Library::for_game(&arctic_core::skins::Library::dir(
            dirs.profile_root(),
        ))
    }

    fn wear_skin(&self, id: Option<&str>) -> arctic_core::Result<()> {
        let tasks = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tasks
            .clone();
        tasks.send(crate::tasks::Event::WearSkinFromGame(id.map(str::to_owned)));
        Ok(())
    }

    fn ffmpeg(&self, start: bool) -> String {
        if crate::video_encoder::available()
            && let Ok(exe) = std::env::current_exe()
        {
            return serde_json::json!({ "state": "ready", "path": exe, "builtin": true })
                .to_string();
        }
        self.capture_encoder(start)
    }

    fn capture_encoder(&self, start: bool) -> String {
        use arctic_core::ffmpeg;
        let dirs = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tasks
            .dirs()
            .clone();
        if let Some(path) = ffmpeg::find(&dirs) {
            return serde_json::json!({ "state": "ready", "path": path,
                "runner": std::env::current_exe().ok() })
            .to_string();
        }
        let mut shared = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let job = &mut shared.ffmpeg;
        if job.running {
            let progress = if job.total > 0 {
                job.done as f64 / job.total as f64
            } else {
                0.0
            };
            return serde_json::json!({ "state": "downloading", "progress": progress }).to_string();
        }
        if !ffmpeg::can_download() {
            return serde_json::json!({
                "state": "unavailable",
                "error": "Install FFmpeg with your package manager, or point Arctic at it in Settings → Video export, then try again."
            })
            .to_string();
        }
        if !start {
            let state = if job.error.is_some() {
                "failed"
            } else {
                "missing"
            };
            return serde_json::json!({ "state": state, "error": job.error }).to_string();
        }
        *job = FfmpegJob {
            running: true,
            ..FfmpegJob::default()
        };
        drop(shared);
        let state = Arc::clone(&self.0);
        std::thread::spawn(move || {
            let report = Arc::clone(&state);
            let result = ffmpeg::install(&dirs, |done, total| {
                let mut s = report.lock().unwrap_or_else(PoisonError::into_inner);
                s.ffmpeg.done = done;
                s.ffmpeg.total = total;
            });
            let mut s = state.lock().unwrap_or_else(PoisonError::into_inner);
            s.ffmpeg.running = false;
            if let Err(e) = result {
                log::warn!("FFmpeg download: {e}");
                s.ffmpeg.error = Some(e.to_string());
            }
        });
        serde_json::json!({ "state": "downloading", "progress": 0.0 }).to_string()
    }

    fn icon(&self, url: &str) -> arctic_core::Result<Vec<u8>> {
        const SIZE: u32 = 64;
        if !url.starts_with("https://cdn.modrinth.com/") {
            return Err(arctic_core::Error::Other(
                t!("tasks.only_modrinth_icons").into(),
            ));
        }
        let tasks = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tasks
            .clone();
        let bytes = arctic_core::mods::icon(url, &tasks.dirs().cache().join("icons"))?;
        let img = image::load_from_memory(&bytes)
            .map_err(|e| arctic_core::Error::Other(tf!("tasks.unreadable_icon", e = e)))?
            .resize(SIZE, SIZE, image::imageops::FilterType::Triangle);
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png)
            .map_err(|e| arctic_core::Error::Other(e.to_string()))?;
        Ok(out.into_inner())
    }

    fn copy_image(&self, path: &str) -> arctic_core::Result<()> {
        let path = std::path::Path::new(path);
        // Only screenshots: a `.png` directly inside a `screenshots` folder.
        let is_png = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"));
        let in_screenshots = path
            .parent()
            .and_then(|d| d.file_name())
            .is_some_and(|n| n.eq_ignore_ascii_case("screenshots"));
        if !is_png || !in_screenshots || !path.is_file() {
            return Err(arctic_core::Error::Other("not a screenshot".into()));
        }
        // The game may still be writing the file, and another app may hold the
        // clipboard for a moment (clipboard history, chat apps): try again
        // for a little while before giving up.
        let img = retry(|| image::open(path).map(|i| i.to_rgba8()))
            .map_err(|e| arctic_core::Error::Other(tf!("tasks.couldnt_read_it", e = e)))?;
        let (width, height) = img.dimensions();
        let bytes = img.into_raw();
        retry(|| {
            let picture = arboard::ImageData {
                width: width as usize,
                height: height as usize,
                bytes: std::borrow::Cow::Borrowed(&bytes),
            };
            arboard::Clipboard::new().and_then(|mut c| c.set_image(picture))
        })
        .map_err(|e| arctic_core::Error::Other(format!("clipboard: {e}")))
    }

    fn login(&self) -> String {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .login_status
            .clone()
    }

    fn voice(&self, state: &[u8]) -> arctic_core::Result<String> {
        let hub = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .voice
            .clone();
        hub.map(|h| h.game_state(state))
            .ok_or_else(|| arctic_core::Error::Other(t!("tasks.voice_chat_isnt_ready").into()))
    }

    fn join(&self, launch: &str) -> Option<String> {
        let mut shared = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let game = shared.games.get_mut(launch)?;
        game.polled = Some(std::time::Instant::now());
        game.join.take()
    }

    fn together(&self, request: &[u8]) -> arctic_core::Result<String> {
        let body: TogetherBody = serde_json::from_slice(request)
            .map_err(|_| arctic_core::Error::Other("bad request".into()))?; // i18n-ignore
        let mut shared = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let wanted = match body.action.as_str() {
            "host" => Some(TogetherRequest::Host),
            "join" if !body.code.trim().is_empty() => {
                Some(TogetherRequest::Join(body.code.trim().to_owned()))
            }
            "stop" => Some(TogetherRequest::Stop),
            "status" => None,
            _ => return Err(arctic_core::Error::Other("unknown action".into())), // i18n-ignore
        };
        if wanted.is_some() {
            shared.together_request = wanted;
        }
        Ok(shared.together_status.clone())
    }
}

impl BridgeHost {
    /// For Settings → Video export: is the launcher's own download of FFmpeg running (and how
    /// far), or did the last one fail?
    pub fn ffmpeg_download(&self) -> (bool, f64, Option<String>) {
        let shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        let job = &shared.ffmpeg;
        let progress = if job.total > 0 {
            job.done as f64 / job.total as f64
        } else {
            0.0
        };
        (job.running, progress, job.error.clone())
    }

    /// For Settings → Video export: start the launcher's own download (Windows).
    pub fn start_ffmpeg_download(&self) {
        let _ = bridge::Accounts::capture_encoder(&AppAccounts(Arc::clone(&self.shared)), true);
    }

    /// Start listening (a failure just means no in-game switching).
    pub fn start(tasks: &Tasks) -> Self {
        let shared = Arc::new(Mutex::new(Shared {
            accounts: Vec::new(),
            active: None,
            tasks: tasks.clone(),
            voice: None,
            together_request: None,
            together_status: r#"{"state":"idle"}"#.into(),
            login_status: r#"{"state":"idle"}"#.into(),
            ffmpeg: FfmpegJob::default(),
            allowed: HashSet::new(),
            switch_ask: None,
            asks_made: 0,
            games: HashMap::new(),
        }));
        let info = bridge::start(AppAccounts(Arc::clone(&shared)))
            .inspect_err(|e| log::warn!("account switcher bridge: {e}"))
            .ok()
            .map(Arc::new);
        Self { info, shared }
    }

    /// Voice chat reports from the game go here.
    pub fn set_voice(&self, hub: crate::voice::VoiceHub) {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .voice = Some(hub);
    }

    /// A game is being launched with `account_id`: it may get that account's session.
    pub fn allow(&self, account_id: &str) {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .allowed
            .insert(account_id.to_owned());
    }

    /// A game run is being launched: its own bridge info, with a token only that game knows.
    pub fn register_launch(&self, run: u64) -> Option<BridgeInfo> {
        let (info, token) = self.info.as_ref()?.for_launch();
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .games
            .insert(
                token,
                GameLink {
                    run,
                    join: None,
                    polled: None,
                },
            );
        Some(info)
    }

    /// A run has ended: its token is no good any more.
    pub fn forget_launch(&self, run: u64) {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .games
            .retain(|_, g| g.run != run);
    }

    /// Queue `address` for the game of `run`, if that game has the Arctic Client listening. False
    /// when it doesn't (never asked lately), so the caller can fall back.
    pub fn send_join(&self, run: u64, address: &str) -> bool {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(game) = shared
            .games
            .values_mut()
            .find(|g| g.run == run && g.polled.is_some_and(|t| t.elapsed() < GAME_ALIVE))
        else {
            return false;
        };
        game.join = Some(address.to_owned());
        true
    }

    /// The name of the account a game wants to switch to and the player has not answered about.
    pub fn switch_question(&self) -> Option<(u64, String)> {
        let shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        shared
            .switch_ask
            .as_ref()
            .filter(|ask| ask.answer.is_none())
            .map(|ask| (ask.serial, ask.name.clone()))
    }

    /// The player's answer to [`Self::switch_question`]. Returns the account's local id when the
    /// answer is a yes the app should remember for good.
    pub fn answer_switch(&self, serial: u64, allow: bool, always: bool) -> Option<String> {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        let ask = shared
            .switch_ask
            .as_mut()
            .filter(|ask| ask.serial == serial)?;
        ask.answer = Some(allow);
        if !allow {
            return None;
        }
        let id = ask.id.clone();
        shared.switch_ask = None;
        shared.allowed.insert(id.clone());
        always.then_some(id)
    }

    /// A play-together request from the game, once.
    pub fn take_together_request(&self) -> Option<TogetherRequest> {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .together_request
            .take()
    }

    /// Play together's state as the game reads it.
    pub fn set_together_status(&self, status: String) {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .together_status = status;
    }

    /// How a sign-in the game started is going (see `Accounts::login`).
    pub fn set_login_status(&self, state: &str, name: Option<&str>, message: Option<&str>) {
        let status = serde_json::json!({ "state": state, "name": name, "message": message });
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .login_status = status.to_string();
    }

    /// The accounts (and tasks, after a profile switch) to answer with.
    pub fn update(&self, store: &AccountStore, tasks: &Tasks) {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        shared.allowed.extend(store.game_switch_ok.iter().cloned());
        shared.accounts = store.accounts.clone();
        shared.active = store.active.clone();
        shared.tasks = tasks.clone();
    }
}

/// How many times, and how far apart, a screenshot copy is tried.
const COPY_ATTEMPTS: u32 = 10;
const COPY_RETRY: std::time::Duration = std::time::Duration::from_millis(100);

/// `f`, tried up to [`COPY_ATTEMPTS`] times; the last error if none worked.
fn retry<T, E>(mut f: impl FnMut() -> Result<T, E>) -> Result<T, E> {
    let mut attempt = 1;
    loop {
        match f() {
            Ok(value) => return Ok(value),
            Err(e) if attempt >= COPY_ATTEMPTS => return Err(e),
            Err(_) => {
                attempt += 1;
                std::thread::sleep(COPY_RETRY);
            }
        }
    }
}
