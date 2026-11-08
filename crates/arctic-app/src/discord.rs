//! Discord Rich Presence: "Playing Arctic Launcher" with what's being
//! played. A background thread owns the IPC connection, retries while
//! Discord isn't running, and only talks to Discord when the presence
//! actually changes.
//!
//! While the game runs, the Arctic Client reports where it is over the
//! launcher's local bridge (the same `/v1/voice` report friends' voice
//! already uses). With "Show server in Discord status" on, that place
//! becomes the top line. The report is read on the UI thread; this module
//! only turns it into text.

use std::net::IpAddr;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

/// Discord application "Arctic Launcher".
const APP_ID: &str = "1553107151306760263";
/// Discord application "Arctic Client", shown while the game runs (Discord
/// titles a presence with its application's name).
const CLIENT_APP_ID: &str = "1554589488825639042";
const LOGO: &str =
    "https://raw.githubusercontent.com/brksfrb/arctic-launcher/main/docs/assets/icon.png";
const SITE: &str = "https://arcticlauncher.com";
/// How often to look for Discord again while it isn't running.
const RETRY: Duration = Duration::from_secs(20);
/// Discord rejects a line longer than this.
const LINE_MAX: usize = 128;
/// What the game calls a world that isn't a multiplayer server.
const SINGLEPLAYER: &str = "Singleplayer";
/// Shown instead of an IP, localhost, or a private/LAN name.
pub(crate) const PRIVATE_SERVER: &str = "Private server";

/// Hover text for the setting that reveals the server.
pub(crate) const SHOW_SERVER_TIP: &str = "While you're in multiplayer, Discord shows the server \
     address on top and keeps the Minecraft version and loader underneath. Singleplayer says \
     Singleplayer; the menus are unchanged. An IP address, localhost, or a LAN address shows as \
     \"Private server\".";

/// What the presence should show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presence {
    pub details: String,
    pub state: Option<String>,
    /// Hover text on the large image.
    pub large_text: String,
    /// Unix milliseconds; Discord shows the elapsed time.
    pub since: i64,
    /// The game is running (shown as the Arctic Client).
    pub in_game: bool,
}

impl Presence {
    fn app_id(&self) -> &'static str {
        if self.in_game { CLIENT_APP_ID } else { APP_ID }
    }
}

/// One launch. `details` is "Minecraft 1.21.11", `state` is "Vanilla".
/// `since` is the session start and does not move when the server changes.
struct GameSession {
    details: String,
    state: String,
    since: i64,
}

pub struct Discord {
    tx: Sender<Option<Presence>>,
    sent: Option<Option<Presence>>,
    launcher_since: i64,
    game: Option<GameSession>,
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
        self.game = Some(GameSession {
            details,
            state,
            since: now_ms(),
        });
    }

    /// Call every frame; sends only when the text actually changed.
    ///
    /// Voice reports arrive many times a second, but an unchanged server
    /// builds an equal [`Presence`], so nothing is written to the worker.
    /// The worker also keeps only the newest request, so a burst of real
    /// changes (leaving a server, joining the next) becomes one IPC call.
    ///
    /// `together` is a play-together note ("Hosting a world"). `place` is
    /// what the game last reported: `None` in the menus, `"Singleplayer"`,
    /// or the address the player connected with. It is used only when
    /// `show_server` is on.
    pub fn sync(
        &mut self,
        enabled: bool,
        playing: bool,
        together: Option<String>,
        show_server: bool,
        place: Option<String>,
    ) {
        if !playing {
            self.game = None;
        }
        let want = presence_for(
            enabled,
            self.game.as_ref(),
            self.launcher_since,
            show_server,
            place.as_deref(),
            together.as_deref(),
        );
        if self.sent.as_ref() != Some(&want) {
            let _ = self.tx.send(want.clone());
            self.sent = Some(want);
        }
    }
}

/// The presence to publish, or `None` when Discord status is off.
fn presence_for(
    enabled: bool,
    game: Option<&GameSession>,
    launcher_since: i64,
    show_server: bool,
    place: Option<&str>,
    together: Option<&str>,
) -> Option<Presence> {
    if !enabled {
        return None;
    }
    Some(match game {
        Some(game) => {
            let lines = in_game_lines(&game.details, &game.state, show_server, place, together);
            Presence {
                details: lines.details,
                state: lines.state,
                large_text: lines.large_text,
                // The session clock, not the moment the server changed.
                since: game.since,
                in_game: true,
            }
        }
        None => Presence {
            details: "In the launcher".into(),
            state: None,
            large_text: "Arctic Launcher".into(),
            since: launcher_since,
            in_game: false,
        },
    })
}

/// The two Discord lines and the image hover for a running game.
///
/// Discord gives a presence two text lines plus hover text on the large
/// image. When the server is shown, the address is the top line (the line
/// LabyMod uses for "Premium.blocksmc.com") and the version and loader share
/// the second ("Minecraft 1.21.11 · Vanilla"), so neither is dropped. A
/// play-together note, which otherwise owns the second line, moves to the
/// image hover so the version stays put. In the menus, and while the setting
/// is off, the lines stay as they are without the setting: version on top,
/// loader or the together note underneath.
fn in_game_lines(
    version: &str,
    loader: &str,
    show_server: bool,
    place: Option<&str>,
    together: Option<&str>,
) -> Lines {
    let place = show_server
        .then_some(place)
        .flatten()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match place {
        Some(raw) => Lines {
            details: clip(&server_label(raw)),
            state: Some(clip(&format!("{version} · {loader}"))),
            large_text: clip(together.unwrap_or("Arctic Client")),
        },
        None => Lines {
            details: clip(version),
            state: Some(clip(together.unwrap_or(loader))),
            large_text: "Arctic Client".into(),
        },
    }
}

struct Lines {
    details: String,
    state: Option<String>,
    large_text: String,
}

/// Top line for a place the game reported.
pub(crate) fn server_label(reported: &str) -> String {
    let reported = reported.trim();
    if reported.eq_ignore_ascii_case(SINGLEPLAYER) {
        SINGLEPLAYER.to_owned()
    } else {
        sanitize_address(reported)
    }
}

/// Hostname Discord may show. Ports are removed. A raw IP (v4 or v6),
/// localhost, or a private/LAN name becomes [`PRIVATE_SERVER`] — the raw
/// address is never returned.
pub(crate) fn sanitize_address(address: &str) -> String {
    let Some(host) = host_of(address) else {
        return PRIVATE_SERVER.to_owned();
    };
    if hidden_host(&host) {
        PRIVATE_SERVER.to_owned()
    } else {
        host
    }
}

/// The hostname the player connected with, lowercased, without a port.
/// `None` when `address` isn't an address (it may still contain an IP, so
/// the caller hides it rather than echoing it).
fn host_of(address: &str) -> Option<String> {
    let text = address.trim();
    if text.is_empty() || text.len() > 300 {
        return None;
    }
    let (host, port) = if let Some(rest) = text.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(p) => p.parse().ok()?,
            None if after.is_empty() => 25565u16,
            None => return None,
        };
        (host, port)
    } else if text.matches(':').count() > 1 {
        // A bare IPv6 address. It is hidden later; parsing it here is what
        // stops the colon from being read as a port.
        (text, 25565)
    } else if let Some((host, port)) = text.split_once(':') {
        (host, port.parse().ok()?)
    } else {
        (text, 25565)
    };
    if host.is_empty() || port == 0 {
        return None;
    }
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host.chars().any(|c| {
            c.is_whitespace() || c.is_control() || matches!(c, '/' | '\\' | '@' | '?' | '#')
        })
    {
        return None;
    }
    Some(host)
}

/// An address that must not be published: any IP literal, localhost, or a
/// name that only resolves on the local network.
fn hidden_host(host: &str) -> bool {
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".lan")
    {
        return true;
    }
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }
    // `fe80::1%eth0` and similar zone ids.
    if let Some((addr, _)) = host.split_once('%')
        && addr.parse::<IpAddr>().is_ok()
    {
        return true;
    }
    // Dotted numbers Rust won't parse as an IP: leading zeros
    // ("192.168.001.001"), a short form ("127.1"), or an out-of-range group.
    host.contains('.')
        && host
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

fn clip(text: &str) -> String {
    if text.chars().count() <= LINE_MAX {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(LINE_MAX - 1).collect();
    out.push('…');
    out
}

fn worker(rx: Receiver<Option<Presence>>) {
    // The connection and the application it speaks for.
    let mut client: Option<(DiscordIpcClient, &str)> = None;
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
            if let Some((mut c, _)) = client.take() {
                let _ = c.clear_activity();
                let _ = c.close();
            }
            shown = None;
            continue;
        };
        let app = presence.app_id();
        if client.as_ref().is_some_and(|(_, id)| *id != app)
            && let Some((mut c, _)) = client.take()
        {
            let _ = c.clear_activity();
            let _ = c.close();
        }
        if client.is_none() {
            let mut c = DiscordIpcClient::new(app);
            if c.connect().is_err() {
                continue; // Discord isn't running; try again later.
            }
            client = Some((c, app));
            shown = None;
        }
        if shown.as_ref() == Some(presence) {
            continue;
        }
        let Some((c, _)) = client.as_mut() else {
            continue;
        };
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
    if let Some((mut c, _)) = client {
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
                .large_text(p.large_text.as_str()),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(
        show: bool,
        place: Option<&str>,
        together: Option<&str>,
    ) -> (String, Option<String>, String) {
        let l = in_game_lines("Minecraft 1.21.11", "Vanilla", show, place, together);
        (l.details, l.state, l.large_text)
    }

    #[test]
    fn menus_and_the_off_switch_keep_todays_lines() {
        let menus = lines(true, None, None);
        let off = lines(false, Some("premium.blocksmc.com"), None);
        let expected = (
            "Minecraft 1.21.11".into(),
            Some("Vanilla".into()),
            "Arctic Client".into(),
        );
        assert_eq!(menus, expected);
        assert_eq!(off, expected);
        // A blank report is the menus too.
        assert_eq!(lines(true, Some("   "), None), expected);
    }

    #[test]
    fn server_is_the_top_line_and_version_stays_under_it() {
        assert_eq!(
            lines(true, Some("Premium.BlocksMC.com"), None),
            (
                "premium.blocksmc.com".into(),
                Some("Minecraft 1.21.11 · Vanilla".into()),
                "Arctic Client".into(),
            )
        );
        assert_eq!(lines(true, Some("Singleplayer"), None).0, "Singleplayer");
    }

    #[test]
    fn together_keeps_the_second_line_until_a_server_is_shown() {
        assert_eq!(
            lines(false, None, Some("Hosting a world")),
            (
                "Minecraft 1.21.11".into(),
                Some("Hosting a world".into()),
                "Arctic Client".into(),
            )
        );
        // With a server on the top line, version and loader stay on the
        // second and the together note moves to the image hover.
        assert_eq!(
            lines(true, Some("mc.example.net"), Some("Hosting a world")),
            (
                "mc.example.net".into(),
                Some("Minecraft 1.21.11 · Vanilla".into()),
                "Hosting a world".into(),
            )
        );
    }

    #[test]
    fn switching_servers_does_not_restart_the_session_clock() {
        let game = GameSession {
            details: "Minecraft 1.21.11".into(),
            state: "Fabric".into(),
            since: 1_700_000_000_000,
        };
        let first =
            presence_for(true, Some(&game), 1, true, Some("one.example.net"), None).unwrap();
        let second = presence_for(
            true,
            Some(&game),
            1,
            true,
            Some("two.example.net:25566"),
            None,
        )
        .unwrap();
        assert_eq!(first.since, game.since);
        assert_eq!(second.since, first.since);
        assert_eq!(first.details, "one.example.net");
        assert_eq!(second.details, "two.example.net");
        assert_eq!(second.state.as_deref(), Some("Minecraft 1.21.11 · Fabric"));
        // The same place is the same presence, so the worker is not told again.
        assert_eq!(
            first,
            presence_for(true, Some(&game), 1, true, Some("one.example.net"), None).unwrap()
        );
    }

    #[test]
    fn hostname_keeps_what_was_typed_and_drops_the_port() {
        assert_eq!(
            sanitize_address("Premium.BlocksMC.com"),
            "premium.blocksmc.com"
        );
        assert_eq!(
            sanitize_address("Premium.BlocksMC.com:25565"),
            "premium.blocksmc.com"
        );
        assert_eq!(sanitize_address(" mc.hypixel.net:25566 "), "mc.hypixel.net");
        assert_eq!(sanitize_address("play.example.com."), "play.example.com");
        assert_eq!(server_label("  singleplayer "), "Singleplayer");
    }

    #[test]
    fn raw_ips_and_private_names_become_private_server() {
        for address in [
            "1.2.3.4",
            "8.8.8.8:25565",
            "192.168.1.20",
            "192.168.1.20:25565",
            "10.0.0.5",
            "172.16.0.9:25565",
            "127.0.0.1",
            "127.0.0.1:25565",
            "169.254.1.1",
            "0.0.0.0",
            "255.255.255.255",
            "192.168.001.001",
            "0177.0.0.1",
            "999.1.2.3",
            "127.1",
            "1.2.3",
            "::1",
            "[::1]",
            "[::1]:25565",
            "2001:db8::5",
            "[2001:db8::5]:25566",
            "fe80::1",
            "fd12:3456:789a::1",
            "::ffff:1.2.3.4",
            "fe80::1%eth0",
            "localhost",
            "LOCALHOST",
            "localhost:25570",
            "foo.localhost",
            "printer.local",
            "house.lan",
            "1.2.3.4:notaport",
            "[::1",
            "",
            "   ",
            "has space.com",
        ] {
            assert_eq!(sanitize_address(address), PRIVATE_SERVER, "{address}");
        }
    }

    #[test]
    fn a_long_hostname_is_cut_to_discords_limit() {
        let host = format!("{}.example.net", "a".repeat(200));
        let (details, _, _) = lines(true, Some(&host), None);
        assert_eq!(details.chars().count(), LINE_MAX);
        assert!(details.ends_with('…'));
        assert!(!details.contains(':'));
    }
}
