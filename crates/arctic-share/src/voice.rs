//! The network side of proximity voice chat: one iroh endpoint that keeps
//! direct (or relayed) connections to the other voice chat members on the
//! same Minecraft server and trades voice packets as unreliable datagrams
//! (a late packet is worse than a lost one). Who to connect to comes from
//! the caller (the Arctic server's room list); only one side of each pair
//! dials (the lower id), the other accepts.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;

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

async fn run(
    endpoint: iroh::Endpoint,
    mut commands: mpsc::UnboundedReceiver<Command>,
    received: Received,
) {
    let me = endpoint.id().to_string();
    let mut wanted: HashSet<String> = HashSet::new();
    let (conn_tx, mut conn_rx) = mpsc::unbounded_channel::<(String, Connection)>();
    let mut connections: HashMap<String, Connection> = HashMap::new();
    let mut dialing: HashSet<String> = HashSet::new();
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
                        for node in &wanted {
                            // The lower id dials; the other side accepts.
                            if node.as_str() > me.as_str()
                                && !connections.contains_key(node)
                                && dialing.insert(node.clone())
                                && let Ok(id) = EndpointId::from_str(node)
                            {
                                let (endpoint, tx, node) = (endpoint.clone(), conn_tx.clone(), node.clone());
                                tokio::spawn(async move {
                                    match endpoint.connect(id, VOICE_ALPN).await {
                                        Ok(c) => {
                                            let _ = tx.send((node, c));
                                        }
                                        Err(e) => log::info!("voice: couldn't reach a member: {e}"),
                                    }
                                });
                            }
                        }
                    }
                }
            }
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else { break };
                let tx = conn_tx.clone();
                tokio::spawn(async move {
                    if let Ok(c) = incoming.await {
                        let node = c.remote_id().to_string();
                        let _ = tx.send((node, c));
                    }
                });
            }
            Some((node, c)) = conn_rx.recv() => {
                dialing.remove(&node);
                // Only members of the room get through.
                if !wanted.contains(&node) {
                    c.close(0u32.into(), b"not in the room");
                    continue;
                }
                let (reader, received, from) = (c.clone(), received.clone(), node.clone());
                tokio::spawn(async move {
                    while let Ok(data) = reader.read_datagram().await {
                        received(&from, &data);
                    }
                });
                log::info!("voice: connected to {node}");
                if let Some(old) = connections.insert(node, c) {
                    old.close(0u32.into(), b"replaced");
                }
            }
        }
    }
    endpoint.close().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;

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
