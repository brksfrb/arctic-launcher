//! Proximity voice chat in the launcher. The game reports (through the
//! bridge, several times a second) which server it's on, where you are and
//! where the players around you are; this joins that server's voice room
//! on the Arctic server, connects directly to the other members, sends
//! your voice and plays theirs placed by position. Nothing runs until voice
//! chat is on and you're on a server; it stops when the game goes quiet.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use arctic_core::auth::Account;
use arctic_core::settings::VoiceSettings;
use arctic_core::storage::DataDirs;
use arctic_core::voice::{self as rooms, Member};
use arctic_share::voice::VoiceNet;
use arctic_voice::engine::{Config, Engine, Mode, PeerId};
use arctic_voice::spatial::Place;
use serde::Deserialize;
use serde_json::json;

use crate::voice_svc::{Frame, GameSvc, SvcSide};

/// Check in to the room this often.
const JOIN_EVERY: Duration = Duration::from_secs(20);
/// No word from the game this long: it closed or left; stop.
const GAME_QUIET: Duration = Duration::from_secs(5);
/// After voice fails to start, wait this long before trying again.
const RETRY_AFTER: Duration = Duration::from_secs(15);

#[derive(Deserialize)]
struct Spot {
    uuid: String,
    x: f64,
    y: f64,
    z: f64,
    #[serde(default)]
    yaw: f64,
    #[serde(default)]
    name: Option<String>,
}

impl Spot {
    fn place(&self) -> Place {
        Place {
            x: self.x,
            y: self.y,
            z: self.z,
            yaw: self.yaw,
        }
    }
}

/// What the game sends.
#[derive(Deserialize)]
struct GameState {
    /// Server address; missing in singleplayer and menus.
    server: Option<String>,
    me: Option<Spot>,
    #[serde(default)]
    players: Vec<Spot>,
    #[serde(default)]
    talking: bool,
    /// Mute or unmute this player (their UUID), picked in game.
    #[serde(default)]
    mute: Option<String>,
    /// Simple Voice Chat's secret, on servers that run it.
    #[serde(default)]
    svc: Option<GameSvc>,
}

struct Session {
    server: String,
    room: String,
    net: Arc<VoiceNet>,
    engine: Arc<Engine>,
    /// Room members by node id, and their peer numbers for the engine.
    members: Arc<Mutex<HashMap<String, (PeerId, Member)>>>,
    joined: Option<Instant>,
    joining: Arc<Mutex<bool>>,
    svc: SvcSide,
}

struct State {
    settings: VoiceSettings,
    account: Option<Account>,
    session: Option<Session>,
    heard_game: Option<Instant>,
    status: String,
    /// The game changed who's muted; the launcher saves it.
    muted_changed: bool,
    /// Voice failed to start; don't try again before this.
    retry_at: Option<Instant>,
}

/// Shared between the launcher's UI and the bridge thread.
#[derive(Clone)]
pub struct VoiceHub {
    dirs: Arc<Mutex<DataDirs>>,
    state: Arc<Mutex<State>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn config(s: &VoiceSettings) -> Config {
    Config {
        input: s.input.clone(),
        output: s.output.clone(),
        mode: if s.push_to_talk {
            Mode::PushToTalk
        } else {
            Mode::Voice {
                threshold_db: s.threshold_db,
            }
        },
        mic_gain: s.mic_gain,
        volume: s.volume,
        ..Config::default()
    }
}

/// Add or remove a UUID (dashless, lower case) from the muted list.
fn toggle_mute(muted: &mut Vec<String>, uuid: &str) {
    let uuid = uuid.replace('-', "").to_ascii_lowercase();
    if uuid.len() != 32 || !uuid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return;
    }
    if let Some(i) = muted.iter().position(|m| *m == uuid) {
        muted.remove(i);
    } else {
        muted.push(uuid);
    }
}

/// A stable number for a node id.
pub(crate) fn peer_id(node: &str) -> PeerId {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    node.hash(&mut h);
    h.finish()
}

impl VoiceHub {
    pub fn new(dirs: DataDirs, settings: VoiceSettings) -> Self {
        Self {
            dirs: Arc::new(Mutex::new(dirs)),
            state: Arc::new(Mutex::new(State {
                settings,
                account: None,
                session: None,
                heard_game: None,
                status: String::new(),
                muted_changed: false,
                retry_at: None,
            })),
        }
    }

    /// The launcher's profile, settings and active account (every frame).
    pub fn update(&self, dirs: &DataDirs, settings: &VoiceSettings, account: Option<&Account>) {
        {
            let mut d = lock(&self.dirs);
            if d.profile_root() != dirs.profile_root() {
                *d = dirs.clone();
            }
        }
        let mut st = lock(&self.state);
        let account_changed = st.account.as_ref().map(|a| &a.id) != account.map(|a| &a.id);
        st.account = account.cloned();
        let mut settings = settings.clone();
        if st.muted_changed {
            // The game's choice wins until the launcher has taken it.
            settings.muted = st.settings.muted.clone();
        }
        let settings = &settings;
        if st.settings != *settings {
            st.settings = settings.clone();
            st.retry_at = None;
            if let Some(s) = &st.session {
                s.engine.set_config(config(settings));
            }
        }
        if !settings.enabled || account_changed {
            self.stop(&mut st);
        }
    }

    /// Who's muted, when the game changed it since last asked.
    pub fn take_muted(&self) -> Option<Vec<String>> {
        let mut st = lock(&self.state);
        if !st.muted_changed {
            return None;
        }
        st.muted_changed = false;
        Some(st.settings.muted.clone())
    }

    /// Stop if the game hasn't reported in a while (call now and then).
    pub fn tick(&self) {
        let mut st = lock(&self.state);
        if st.session.is_some() && st.heard_game.is_none_or(|t| t.elapsed() > GAME_QUIET) {
            self.stop(&mut st);
        }
    }

    /// What the voice section shows ("Connected to 3 players" …).
    pub fn status(&self) -> String {
        let st = lock(&self.state);
        match &st.session {
            Some(s) if s.engine.mic_error().is_some() => {
                "Listening only: Arctic can't use your microphone. Check Windows Settings →                  Privacy → Microphone (\"Let desktop apps access your microphone\"), or pick                  another microphone below."
                    .into()
            }
            Some(s) => {
                let n = lock(&s.members).len();
                if n == 0 {
                    "On, nobody else here uses Arctic voice yet".into()
                } else {
                    format!("On, {n} other player(s) in voice on this server")
                }
            }
            None if !st.status.is_empty() => st.status.clone(),
            None if st.settings.enabled => "On: starts when you join a server".into(),
            None => "Off".into(),
        }
    }

    /// Microphone level for a meter, while running.
    pub fn mic_level(&self) -> Option<f32> {
        lock(&self.state)
            .session
            .as_ref()
            .map(|s| s.engine.mic_level())
    }

    /// The game's report; the answer tells it who's speaking.
    pub fn game_state(&self, body: &[u8]) -> String {
        let Ok(game) = serde_json::from_slice::<GameState>(body) else {
            return json!({"error": "bad state"}).to_string();
        };
        let mut st = lock(&self.state);
        st.heard_game = Some(Instant::now());
        if let Some(uuid) = game.mute.as_deref() {
            toggle_mute(&mut st.settings.muted, uuid);
            st.muted_changed = true;
        }
        let server = game.server.filter(|s| !s.is_empty());
        let (Some(server), Some(me), true) = (server, game.me.as_ref(), st.settings.enabled) else {
            self.stop(&mut st);
            return json!({"enabled": st.settings.enabled, "active": false}).to_string();
        };
        if st.session.as_ref().is_some_and(|s| s.server != server) {
            self.stop(&mut st);
        }
        if st.session.is_none() {
            if st.retry_at.is_some_and(|t| Instant::now() < t) {
                return json!({"enabled": true, "active": false, "error": st.status}).to_string();
            }
            match self.start(&st.settings, &server) {
                Ok(s) => {
                    st.status.clear();
                    st.retry_at = None;
                    st.session = Some(s);
                }
                Err(e) => {
                    log::warn!("voice chat couldn't start: {e}");
                    st.retry_at = Some(Instant::now() + RETRY_AFTER);
                    st.status = format!("Voice chat couldn't start: {e}");
                    return json!({"enabled": true, "active": false, "error": st.status})
                        .to_string();
                }
            }
        }
        let settings = st.settings.clone();
        let account = st.account.clone();
        let Some(session) = st.session.as_mut() else {
            return json!({"enabled": true, "active": false}).to_string();
        };
        // Check in now and then (in the background).
        let due = session.joined.is_none_or(|t| t.elapsed() > JOIN_EVERY);
        if due && !*lock(&session.joining) {
            *lock(&session.joining) = true;
            session.joined = Some(Instant::now());
            self.check_in(session, &settings, account, me.uuid.clone());
        }
        // Simple Voice Chat, when wanted and the server runs it.
        let svc_wanted = settings.simple_voice_chat && !settings.friends_only;
        let engine = session.engine.clone();
        session
            .svc
            .update(game.svc.as_ref().filter(|_| svc_wanted), &engine);
        // Positions.
        let engine = &session.engine;
        engine.set_listener(Some(me.place()));
        engine.set_talking(game.talking);
        let by_uuid: HashMap<String, &Spot> = game
            .players
            .iter()
            .map(|p| (p.uuid.replace('-', "").to_ascii_lowercase(), p))
            .collect();
        let members = lock(&session.members);
        let mut speaking_uuids = Vec::new();
        let mut listed = Vec::new();
        let speaking = engine.speaking();
        for (peer, member) in members.values() {
            let place = by_uuid.get(&member.uuid).map(|p| p.place());
            engine.set_place(*peer, place);
            let muted = settings.muted.iter().any(|m| m == &member.uuid);
            engine.set_volume(*peer, if muted { 0.0 } else { 1.0 });
            let talking = speaking.contains(peer) && !muted;
            if talking {
                speaking_uuids.push(member.uuid.clone());
            }
            listed.push(json!({
                "uuid": member.uuid,
                "name": member.name,
                "friend": member.friend,
                "muted": muted,
                "speaking": talking,
                "near": place.is_some(),
            }));
        }
        let frame = Frame {
            listener: Some(me.place()),
            places: by_uuid
                .iter()
                .map(|(k, p)| (k.clone(), p.place()))
                .collect(),
            names: by_uuid
                .iter()
                .filter_map(|(k, p)| Some((k.clone(), p.name.clone()?)))
                .collect(),
            muted: settings.muted.clone(),
            arctic: members.values().map(|(_, m)| m.uuid.clone()).collect(),
        };
        for entry in session.svc.tick(engine, &frame) {
            if entry["speaking"] == true
                && let Some(uuid) = entry["uuid"].as_str()
            {
                speaking_uuids.push(uuid.to_owned());
            }
            listed.push(entry);
        }
        json!({
            "enabled": true,
            "active": true,
            "sending": engine.sending(),
            "speaking": speaking_uuids,
            "members": listed,
            "svc": svc_wanted,
            "listenOnly": engine.mic_error().is_some(),
            "svcConnected": session.svc.connected(),
        })
        .to_string()
    }

    fn start(&self, settings: &VoiceSettings, server: &str) -> Result<Session, String> {
        let members: Arc<Mutex<HashMap<String, (PeerId, Member)>>> = Arc::default();
        let slot: Arc<Mutex<Option<Arc<Engine>>>> = Arc::default();
        let (known, engine_slot) = (members.clone(), slot.clone());
        let net = Arc::new(VoiceNet::start(Arc::new(
            move |node: &str, data: &[u8]| {
                // Only room members are heard.
                let peer = lock(&known).get(node).map(|(p, _)| *p);
                if let (Some(peer), Some(engine)) = (peer, lock(&engine_slot).clone()) {
                    engine.receive(peer, data);
                }
            },
        ))?);
        let sender = net.clone();
        let svc = SvcSide::default();
        let svc_sender = svc.sender();
        let engine = Arc::new(Engine::start(
            config(settings),
            Box::new(move |p| {
                svc_sender(&p);
                sender.send(p);
            }),
        )?);
        *lock(&slot) = Some(engine.clone());
        Ok(Session {
            server: server.to_owned(),
            room: rooms::room_for(server),
            net,
            engine,
            members,
            joined: None,
            joining: Arc::default(),
            svc,
        })
    }

    fn check_in(
        &self,
        session: &Session,
        settings: &VoiceSettings,
        account: Option<Account>,
        playing_as: String,
    ) {
        let Some(account) = account else {
            *lock(&session.joining) = false;
            return;
        };
        let (dirs, room, node) = (
            lock(&self.dirs).clone(),
            session.room.clone(),
            session.net.node().to_owned(),
        );
        let (members, net, joining, engine) = (
            session.members.clone(),
            session.net.clone(),
            session.joining.clone(),
            session.engine.clone(),
        );
        let friends_only = settings.friends_only;
        std::thread::spawn(move || {
            let base = arctic_core::cosmetics::base_url();
            let result = arctic_core::cosmetics::token_for(&dirs, &base, &account)
                .and_then(|token| rooms::join(&base, &token, &room, &node, &playing_as));
            match result {
                Ok(list) => {
                    let list: Vec<Member> = list
                        .into_iter()
                        .filter(|m| m.friend || !friends_only)
                        .collect();
                    let mut known = lock(&members);
                    let gone: Vec<String> = known
                        .keys()
                        .filter(|k| !list.iter().any(|m| &m.node == *k))
                        .cloned()
                        .collect();
                    for node in gone {
                        if let Some((peer, _)) = known.remove(&node) {
                            engine.remove(peer);
                        }
                    }
                    for m in list {
                        known.insert(m.node.clone(), (peer_id(&m.node), m));
                    }
                    net.set_peers(known.keys().cloned().collect::<Vec<_>>());
                }
                Err(e) => log::info!("voice room: {e}"),
            }
            *lock(&joining) = false;
        });
    }

    fn stop(&self, st: &mut State) {
        if st.session.take().is_some()
            && let Some(account) = st.account.clone()
        {
            let dirs = lock(&self.dirs).clone();
            std::thread::spawn(move || {
                let base = arctic_core::cosmetics::base_url();
                if let Ok(token) = arctic_core::cosmetics::token_for(&dirs, &base, &account) {
                    let _ = rooms::leave(&base, &token);
                }
            });
        }
    }
}

/// Microphones and outputs for the settings.
pub fn devices() -> (Vec<String>, Vec<String>) {
    arctic_voice::engine::devices()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_mute_adds_then_removes_normalized_uuid() {
        let mut muted = Vec::new();
        toggle_mute(&mut muted, "0F0E0D0C-0B0A-4908-8706-050403020100");
        assert_eq!(muted, vec!["0f0e0d0c0b0a49088706050403020100".to_string()]);
        toggle_mute(&mut muted, "0f0e0d0c0b0a49088706050403020100");
        assert!(muted.is_empty());
    }

    #[test]
    fn toggle_mute_ignores_garbage() {
        let mut muted = Vec::new();
        toggle_mute(&mut muted, "not-a-uuid");
        assert!(muted.is_empty());
    }

    #[test]
    fn game_mute_is_kept_until_the_launcher_takes_it() {
        let dir = std::env::temp_dir().join(format!("arctic-voice-test-{}", std::process::id()));
        let dirs = DataDirs::new(&dir);
        let settings = VoiceSettings::default();
        let hub = VoiceHub::new(dirs.clone(), settings.clone());
        hub.game_state(br#"{"mute":"0f0e0d0c0b0a49088706050403020100"}"#);
        // The launcher's (older) settings must not undo the game's change.
        hub.update(&dirs, &settings, None);
        let taken = hub.take_muted().expect("changed");
        assert_eq!(taken, vec!["0f0e0d0c0b0a49088706050403020100".to_string()]);
        assert!(hub.take_muted().is_none());
    }
}
