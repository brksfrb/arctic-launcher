//! The network side of proximity voice chat: one iroh endpoint that keeps
//! direct (or relayed) connections to the other voice chat members on the
//! same Minecraft server and trades voice packets as unreliable datagrams
//! (a late packet is worse than a lost one). Who to connect to comes from
//! the caller (the Arctic server's room list); only one side of each pair
//! dials (the lower id), the other accepts.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use iroh::EndpointId;
use iroh::endpoint::Connection;
use tokio::sync::mpsc;

const VOICE_ALPN: &[u8] = b"arctic/voice/1";

/// A voice packet arrived: (sender's node id, bytes).
pub type Received = Arc<dyn Fn(&str, &[u8]) + Send + Sync>;

enum Command {
    /// Keep connections to exactly these node ids.
    Peers(HashSet<String>),
    Send(Vec<u8>),
}

/// Running voice networking; dropping it disconnects.
pub struct VoiceNet {
    node: String,
    commands: mpsc::UnboundedSender<Command>,
    _runtime: tokio::runtime::Runtime,
}

impl VoiceNet {
    pub fn start(received: Received) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("arctic-voice-net")
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let endpoint = runtime
            .block_on(
                crate::endpoint_builder()
                    .alpns(vec![VOICE_ALPN.to_vec()])
                    .bind(),
            )
            .map_err(|e| format!("couldn't start voice networking: {e}"))?;
        let node = endpoint.id().to_string();
        let (tx, rx) = mpsc::unbounded_channel();
        runtime.spawn(run(endpoint, rx, received));
        Ok(Self {
            node,
            commands: tx,
            _runtime: runtime,
        })
    }

    /// This launcher's id, for the Arctic server's room list.
    pub fn node(&self) -> &str {
        &self.node
    }

    /// Connect to these members (and drop anyone else).
    pub fn set_peers(&self, nodes: impl IntoIterator<Item = String>) {
        let _ = self
            .commands
            .send(Command::Peers(nodes.into_iter().collect()));
    }

    /// Send a voice packet to every connected member.
    pub fn send(&self, packet: Vec<u8>) {
        let _ = self.commands.send(Command::Send(packet));
    }
}

/// What the networking loop hears from its tasks.
enum Event {
    /// A connection is up (dialed or accepted).
    Connected(String, Connection),
    /// Dialing the node failed.
    DialFailed(String),
    /// A connection (by `stable_id`) ended.
    Closed(String, usize),
    /// A wait after a failure is over: dial whoever is still missing.
    Redial,
}

/// A connection that lived this long was fine; the next drop starts the retry delays over.
const HEALTHY_AFTER: Duration = Duration::from_secs(30);
const RETRY_FIRST: Duration = Duration::from_secs(1);
const RETRY_LONGEST: Duration = Duration::from_secs(60);

/// The wait before dial number `failures` (1, 2, 3...) is tried: 1 s, doubling, up to a minute.
fn retry_delay(failures: u32) -> Duration {
    RETRY_FIRST
        .saturating_mul(1u32 << failures.saturating_sub(1).min(6))
        .min(RETRY_LONGEST)
}

/// Dial every wanted member that has no connection and isn't being dialed. The lower id dials; the
/// other side accepts.
fn dial_missing(
    me: &str,
    wanted: &HashSet<String>,
    connections: &HashMap<String, Connection>,
    dialing: &mut HashSet<String>,
    endpoint: &iroh::Endpoint,
    events: &mpsc::UnboundedSender<Event>,
) {
    for node in wanted {
        if node.as_str() > me
            && !connections.contains_key(node)
            && dialing.insert(node.clone())
            && let Ok(id) = EndpointId::from_str(node)
        {
            let (endpoint, events, node) = (endpoint.clone(), events.clone(), node.clone());
            tokio::spawn(async move {
                match endpoint.connect(id, VOICE_ALPN).await {
                    Ok(c) => {
                        let _ = events.send(Event::Connected(node, c));
                    }
                    Err(e) => {
                        log::info!("voice: couldn't reach a member: {e}");
                        let _ = events.send(Event::DialFailed(node));
                    }
                }
            });
        }
    }
}

async fn run(
    endpoint: iroh::Endpoint,
    mut commands: mpsc::UnboundedReceiver<Command>,
    received: Received,
) {
    let me = endpoint.id().to_string();
    let mut wanted: HashSet<String> = HashSet::new();
    let (events_tx, mut events) = mpsc::unbounded_channel::<Event>();
    let mut connections: HashMap<String, Connection> = HashMap::new();
    let mut dialing: HashSet<String> = HashSet::new();
    // Failures in a row per member, and when each connection came up.
    let mut failures: HashMap<String, u32> = HashMap::new();
    let mut up_since: HashMap<String, Instant> = HashMap::new();
    loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    None => break,
                    Some(Command::Send(bytes)) => {
                        let data = bytes::Bytes::from(bytes);
                        for c in connections.values() {
                            let _ = c.send_datagram(data.clone());
                        }
                    }
                    Some(Command::Peers(peers)) => {
                        wanted = peers;
                        connections.retain(|node, c| {
                            let keep = wanted.contains(node);
                            if !keep {
                                c.close(0u32.into(), b"left");
                            }
                            keep
                        });
                        failures.retain(|node, _| wanted.contains(node));
                        dial_missing(&me, &wanted, &connections, &mut dialing, &endpoint, &events_tx);
                    }
                }
            }
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else { break };
                let tx = events_tx.clone();
                tokio::spawn(async move {
                    if let Ok(c) = incoming.await {
                        let node = c.remote_id().to_string();
                        let _ = tx.send(Event::Connected(node, c));
                    }
                });
            }
            Some(event) = events.recv() => {
                match event {
                    Event::Connected(node, c) => {
                        dialing.remove(&node);
                        // Only members of the room get through.
                        if !wanted.contains(&node) {
                            c.close(0u32.into(), b"not in the room");
                            continue;
                        }
                        let (reader, received, from) = (c.clone(), received.clone(), node.clone());
                        let tx = events_tx.clone();
                        tokio::spawn(async move {
                            while let Ok(data) = reader.read_datagram().await {
                                received(&from, &data);
                            }
                            let _ = tx.send(Event::Closed(from, reader.stable_id()));
                        });
                        log::info!("voice: connected to {node}");
                        up_since.insert(node.clone(), Instant::now());
                        if let Some(old) = connections.insert(node, c) {
                            old.close(0u32.into(), b"replaced");
                        }
                    }
                    Event::DialFailed(node) => {
                        dialing.remove(&node);
                        retry_later(&node, &mut failures, &events_tx);
                    }
                    Event::Closed(node, id) => {
                        // A connection already replaced by a newer one is not news.
                        if connections.get(&node).is_none_or(|c| c.stable_id() != id) {
                            continue;
                        }
                        connections.remove(&node);
                        log::info!("voice: connection to {node} ended");
                        if up_since.remove(&node).is_some_and(|t| t.elapsed() > HEALTHY_AFTER) {
                            failures.remove(&node);
                        }
                        // The lower id dials; the other side waits for it, and dialing a member that is
                        // gone just fails again, slower each time.
                        if wanted.contains(&node) && node.as_str() > me.as_str() {
                            retry_later(&node, &mut failures, &events_tx);
                        }
                    }
                    Event::Redial => {
                        dial_missing(&me, &wanted, &connections, &mut dialing, &endpoint, &events_tx);
                    }
                }
            }
        }
    }
    endpoint.close().await;
}

/// Count a failure for `node` and dial again after the wait that failure count calls for.
fn retry_later(
    node: &str,
    failures: &mut HashMap<String, u32>,
    events: &mpsc::UnboundedSender<Event>,
) {
    let count = failures.entry(node.to_owned()).or_insert(0);
    *count += 1;
    let (delay, events) = (retry_delay(*count), events.clone());
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        let _ = events.send(Event::Redial);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn retries_wait_longer_each_time_up_to_a_minute() {
        assert_eq!(retry_delay(1), Duration::from_secs(1));
        assert_eq!(retry_delay(3), Duration::from_secs(4));
        assert_eq!(retry_delay(40), Duration::from_secs(60));
    }

    /// Two launchers in a room hear each other.
    #[test]
    #[ignore = "needs network (iroh relays); run with --ignored"]
    fn two_members_trade_packets() {
        let got: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = got.clone();
        let a = VoiceNet::start(Arc::new(move |_, d: &[u8]| {
            sink.lock().unwrap().push(d.to_vec())
        }))
        .unwrap();
        let b = VoiceNet::start(Arc::new(|_, _: &[u8]| {})).unwrap();
        a.set_peers([b.node().to_owned()]);
        b.set_peers([a.node().to_owned()]);
        let _ = env_logger::builder().is_test(true).try_init();
        for _ in 0..300 {
            b.send(vec![1, 2, 3]);
            std::thread::sleep(Duration::from_millis(100));
            if !got.lock().unwrap().is_empty() {
                break;
            }
        }
        assert_eq!(got.lock().unwrap().first(), Some(&vec![1, 2, 3]));
    }
}
