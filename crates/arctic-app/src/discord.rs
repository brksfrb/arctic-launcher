//! Discord Rich Presence: "Playing Arctic Launcher" with what's being
//! played. A background thread owns the IPC connection, retries while
//! Discord isn't running, and only talks to Discord when the presence
//! actually changes.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

/// Discord application "Arctic Launcher".
const APP_ID: &str = "1553107151306760263";
const LOGO: &str =
    "https://raw.githubusercontent.com/brksfrb/arctic-launcher/main/docs/assets/icon.png";
const SITE: &str = "https://arcticlauncher.com";
/// How often to look for Discord again while it isn't running.
const RETRY: Duration = Duration::from_secs(20);

/// What the presence should show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presence {
    pub details: String,
    pub state: Option<String>,
    /// Unix milliseconds; Discord shows the elapsed time.
    pub since: i64,
}

pub struct Discord {
    tx: Sender<Option<Presence>>,
    sent: Option<Option<Presence>>,
    launcher_since: i64,
    game: Option<Presence>,
}

impl Discord {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || worker(rx));
        if let Err(e) = spawned {
            log::warn!("discord presence thread: {e}");
        }
        Self {
            tx,
            sent: None,
            launcher_since: now_ms(),
            game: None,
        }
    }

    /// A launch started: `details` like "Minecraft 1.21.4", `state` like "Fabric".
    pub fn game_started(&mut self, details: String, state: String) {
        self.game = Some(Presence {
            details,
            state: Some(state),
            since: now_ms(),
        });
    }

    /// Call every frame; sends only changes. `together` replaces the second
    /// line while playing with friends (e.g. "Hosting a world").
    pub fn sync(&mut self, enabled: bool, playing: bool, together: Option<String>) {
        if !playing {
            self.game = None;
        }
        let want = enabled.then(|| {
            let mut presence = self.game.clone().unwrap_or_else(|| Presence {
                details: "In the launcher".into(),
                state: None,
                since: self.launcher_since,
            });
            if let Some(note) = together {
                presence.state = Some(note);
            }
            presence
        });
        if self.sent.as_ref() != Some(&want) {
            let _ = self.tx.send(want.clone());
            self.sent = Some(want);
        }
    }
}

fn worker(rx: Receiver<Option<Presence>>) {
    let mut client: Option<DiscordIpcClient> = None;
    let mut want: Option<Presence> = None;
    let mut shown: Option<Presence> = None;
    loop {
        match rx.recv_timeout(RETRY) {
            Ok(next) => want = next,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        // Only the newest request matters.
        while let Ok(next) = rx.try_recv() {
            want = next;
        }
        let Some(presence) = &want else {
            if let Some(mut c) = client.take() {
                let _ = c.clear_activity();
                let _ = c.close();
            }
            shown = None;
            continue;
        };
        if client.is_none() {
            let mut c = DiscordIpcClient::new(APP_ID);
            if c.connect().is_err() {
                continue; // Discord isn't running; try again later.
            }
            client = Some(c);
            shown = None;
        }
        if shown.as_ref() == Some(presence) {
            continue;
        }
        let Some(c) = client.as_mut() else { continue };
        // Read Discord's reply so errors are visible and replies don't pile up.
        match c.set_activity(activity(presence)).and_then(|()| c.recv()) {
            Ok((_, reply)) => {
                if reply["evt"] == "ERROR" {
                    log::warn!("discord presence rejected: {}", reply["data"]);
                }
                shown = Some(presence.clone());
            }
            Err(e) => {
                log::debug!("discord presence: {e}");
                let _ = c.close();
                client = None;
            }
        }
    }
    if let Some(mut c) = client {
        let _ = c.close();
    }
}

fn activity(p: &Presence) -> Activity<'_> {
    let mut activity = Activity::new()
        .activity_type(ActivityType::Playing)
        .details(p.details.as_str())
        .timestamps(Timestamps::new().start(p.since))
        .assets(
            Assets::new()
                .large_image(LOGO)
                .large_text("Arctic Launcher"),
        )
        .buttons(vec![Button::new("Get Arctic Launcher", SITE)]);
    if let Some(state) = &p.state {
        activity = activity.state(state.as_str());
    }
    activity
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}
