//! The launcher side of the in-game account switcher (see
//! `arctic_core::bridge`): answers the running game with this profile's
//! accounts, refreshing logins through the same task path as launches so
//! the app's account list stays the one source of truth.

use std::sync::{Arc, Mutex, PoisonError};

use arctic_core::auth::{Account, AccountKind, AccountStore};
use arctic_core::bridge::{self, AccountEntry, BridgeInfo, SessionGrant};
use arctic_core::cosmetics;

use crate::tasks::Tasks;

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
}

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
        use arctic_core::ffmpeg;
        let dirs = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tasks
            .dirs()
            .clone();
        // Windows' own encoder, run through this launcher: nothing to fetch.
        if crate::video_encoder::available()
            && let Ok(exe) = std::env::current_exe()
        {
            return serde_json::json!({ "state": "ready", "path": exe, "builtin": true })
                .to_string();
        }
        if let Some(path) = ffmpeg::find(&dirs) {
            return serde_json::json!({ "state": "ready", "path": path }).to_string();
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
                "error": "Install FFmpeg with your package manager, then try again."
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
            return Err(arctic_core::Error::Other("only Modrinth icons".into()));
        }
        let tasks = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tasks
            .clone();
        let bytes = arctic_core::mods::icon(url, &tasks.dirs().cache().join("icons"))?;
        let img = image::load_from_memory(&bytes)
            .map_err(|e| arctic_core::Error::Other(format!("unreadable icon: {e}")))?
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
        let img = image::open(path)
            .map_err(|e| arctic_core::Error::Other(format!("couldn't read it: {e}")))?
            .to_rgba8();
        let (width, height) = img.dimensions();
        let picture = arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Owned(img.into_raw()),
        };
        arboard::Clipboard::new()
            .and_then(|mut c| c.set_image(picture))
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
            .ok_or_else(|| arctic_core::Error::Other("voice chat isn't ready".into()))
    }

    fn together(&self, request: &[u8]) -> arctic_core::Result<String> {
        let body: TogetherBody = serde_json::from_slice(request)
            .map_err(|_| arctic_core::Error::Other("bad request".into()))?;
        let mut shared = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let wanted = match body.action.as_str() {
            "host" => Some(TogetherRequest::Host),
            "join" if !body.code.trim().is_empty() => {
                Some(TogetherRequest::Join(body.code.trim().to_owned()))
            }
            "stop" => Some(TogetherRequest::Stop),
            "status" => None,
            _ => return Err(arctic_core::Error::Other("unknown action".into())),
        };
        if wanted.is_some() {
            shared.together_request = wanted;
        }
        Ok(shared.together_status.clone())
    }
}

impl BridgeHost {
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
        shared.accounts = store.accounts.clone();
        shared.active = store.active.clone();
        shared.tasks = tasks.clone();
    }
}
