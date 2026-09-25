//! Hosting: find the LAN world on this machine and let friends' tunnels
//! reach it.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use iroh::SecretKey;
use iroh::endpoint::{Connection, Incoming, RecvStream, SendStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{oneshot, watch};

use crate::lan::{self, LanWorld};
use crate::{ALPN, GUEST_SUFFIX, KIND_GAME, KIND_INFO, ShareEvent, Sink, code_for};

/// A LAN world counts as closed after this long without announcements.
const WORLD_TIMEOUT: Duration = Duration::from_secs(6);

pub async fn run(key: SecretKey, port: Option<u16>, sink: Sink, mut stop: oneshot::Receiver<()>) {
    let endpoint = match crate::endpoint_builder()
        .secret_key(key)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
    {
        Ok(e) => e,
        Err(e) => {
            sink(ShareEvent::Stopped {
                error: Some(format!("Could not start sharing: {e}")),
            });
            return;
        }
    };
    sink(ShareEvent::HostReady {
        code: code_for(&endpoint.id()),
    });
    let (world_tx, world_rx) = watch::channel(None);
    let detector = match port {
        Some(port) => {
            let world = LanWorld {
                motd: format!("Port {port}"),
                port,
            };
            world_tx.send_replace(Some(world.clone()));
            sink(ShareEvent::HostWorld(Some(world)));
            None
        }
        None => Some(tokio::spawn(detect(world_tx, sink.clone()))),
    };
    let guests = Arc::new(AtomicUsize::new(0));
    loop {
        tokio::select! {
            _ = &mut stop => break,
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else { break };
                tokio::spawn(serve(incoming, world_rx.clone(), guests.clone(), sink.clone()));
            }
        }
    }
    if let Some(task) = detector {
        task.abort();
    }
    endpoint.close().await;
    sink(ShareEvent::Stopped { error: None });
}

/// Watch for "Open to LAN" announcements from this machine.
async fn detect(world: watch::Sender<Option<LanWorld>>, sink: Sink) {
    let socket = match lan::listener() {
        Ok(s) => s,
        Err(e) => {
            log::warn!("LAN listener: {e}");
            return;
        }
    };
    let mut buf = [0u8; 512];
    let mut last_seen: Option<Instant> = None;
    loop {
        let received =
            tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buf)).await;
        if let Ok(Ok((n, _))) = received
            && let Some(found) = lan::parse(&String::from_utf8_lossy(&buf[..n]))
            && !found.motd.ends_with(GUEST_SUFFIX)
        {
            let known = world.borrow().as_ref() == Some(&found);
            if known || lan::is_local(found.port).await {
                last_seen = Some(Instant::now());
                if !known {
                    world.send_replace(Some(found.clone()));
                    sink(ShareEvent::HostWorld(Some(found)));
                }
            }
        }
        if last_seen.is_some_and(|t| t.elapsed() > WORLD_TIMEOUT) {
            last_seen = None;
            world.send_replace(None);
            sink(ShareEvent::HostWorld(None));
        }
    }
}

/// One friend: each stream they open is a world-info request or a game
/// connection.
async fn serve(
    incoming: Incoming,
    world: watch::Receiver<Option<LanWorld>>,
    guests: Arc<AtomicUsize>,
    sink: Sink,
) {
    let conn: Connection = match incoming.await {
        Ok(c) => c,
        Err(e) => {
            log::debug!("share: incoming failed: {e}");
            return;
        }
    };
    sink(ShareEvent::Guests(
        guests.fetch_add(1, Ordering::SeqCst) + 1,
    ));
    while let Ok((send, recv)) = conn.accept_bi().await {
        tokio::spawn(stream(send, recv, world.clone()));
    }
    sink(ShareEvent::Guests(
        guests.fetch_sub(1, Ordering::SeqCst) - 1,
    ));
}

async fn stream(
    mut send: SendStream,
    mut recv: RecvStream,
    world: watch::Receiver<Option<LanWorld>>,
) {
    let mut kind = [0u8; 1];
    if AsyncReadExt::read_exact(&mut recv, &mut kind)
        .await
        .is_err()
    {
        return;
    }
    let current = world.borrow().clone();
    match (kind[0], current) {
        (KIND_INFO, world) => {
            let motd = world.map_or_else(String::new, |w| w.motd);
            let _ = AsyncWriteExt::write_all(&mut send, motd.as_bytes()).await;
            let _ = send.finish();
        }
        (KIND_GAME, Some(world)) => {
            match TcpStream::connect((Ipv4Addr::LOCALHOST, world.port)).await {
                Ok(mut tcp) => {
                    let _ = tcp.set_nodelay(true);
                    let mut tunnel = tokio::io::join(recv, send);
                    let _ = tokio::io::copy_bidirectional(&mut tcp, &mut tunnel).await;
                }
                Err(e) => log::debug!("share: LAN world unreachable: {e}"),
            }
        }
        _ => {
            let _ = send.finish();
        }
    }
}
