//! Joining: connect to the host, then pretend to be a LAN world on this
//! machine so the game lists it under "Local network".

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use iroh::EndpointId;
use iroh::endpoint::Connection;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

use crate::lan;
use crate::{ALPN, GUEST_SUFFIX, KIND_GAME, KIND_INFO, ShareEvent, Sink};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_MOTD: usize = 256;

pub async fn run(host: EndpointId, sink: Sink, mut stop: oneshot::Receiver<()>) {
    let error = session(host, &sink, &mut stop).await.err();
    sink(ShareEvent::Stopped { error });
}

async fn session(
    host: EndpointId,
    sink: &Sink,
    stop: &mut oneshot::Receiver<()>,
) -> Result<(), String> {
    let endpoint = crate::endpoint_builder()
        .bind()
        .await
        .map_err(|e| format!("Could not start networking: {e}"))?;
    let connect = tokio::time::timeout(CONNECT_TIMEOUT, endpoint.connect(host, ALPN));
    let connected = tokio::select! {
        _ = &mut *stop => {
            endpoint.close().await;
            return Ok(());
        }
        connected = connect => connected,
    };
    let conn = match connected {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            log::warn!("share: connect failed: {e:?}");
            return Err(
                "Couldn't reach your friend. Check the code and make sure they're sharing.".into(),
            );
        }
        Err(_) => {
            return Err(
                "Couldn't reach your friend. Check the code and make sure they're sharing.".into(),
            );
        }
    };
    let motd = world_name(&conn).await;
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))
        .await
        .map_err(|e| format!("Could not open a local port: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    sink(ShareEvent::Joined {
        motd: motd.clone(),
        port,
    });
    let announcer = tokio::spawn(announce(format!("{motd}{GUEST_SUFFIX}"), port));
    let result = serve(&conn, &listener, stop).await;
    announcer.abort();
    conn.close(0u32.into(), b"bye");
    endpoint.close().await;
    result
}

async fn world_name(conn: &Connection) -> String {
    let fallback = || "Friend's world".to_owned();
    let Ok((mut send, mut recv)) = conn.open_bi().await else {
        return fallback();
    };
    if AsyncWriteExt::write_all(&mut send, &[KIND_INFO])
        .await
        .is_err()
    {
        return fallback();
    }
    let _ = send.finish();
    let mut buf = Vec::new();
    let _ = (&mut recv)
        .take(MAX_MOTD as u64)
        .read_to_end(&mut buf)
        .await;
    let name = String::from_utf8_lossy(&buf).trim().to_owned();
    if name.is_empty() { fallback() } else { name }
}

/// Forward every local game connection through the tunnel until the host
/// goes away.
async fn serve(
    conn: &Connection,
    listener: &TcpListener,
    stop: &mut oneshot::Receiver<()>,
) -> Result<(), String> {
    loop {
        tokio::select! {
            _ = &mut *stop => return Ok(()),
            reason = conn.closed() => {
                log::info!("share: host closed: {reason}");
                return Err("Your friend stopped sharing.".into());
            }
            accepted = listener.accept() => {
                let Ok((tcp, peer)) = accepted else { continue };
                if !from_this_machine(&tcp, peer) {
                    continue;
                }
                let conn = conn.clone();
                tokio::spawn(forward(conn, tcp));
            }
        }
    }
}

/// Only games on this PC may use the tunnel, not other devices on the LAN.
fn from_this_machine(tcp: &TcpStream, peer: SocketAddr) -> bool {
    peer.ip().is_loopback() || tcp.local_addr().is_ok_and(|local| local.ip() == peer.ip())
}

async fn forward(conn: Connection, mut tcp: TcpStream) {
    let Ok((mut send, recv)) = conn.open_bi().await else {
        return;
    };
    if AsyncWriteExt::write_all(&mut send, &[KIND_GAME])
        .await
        .is_err()
    {
        return;
    }
    let _ = tcp.set_nodelay(true);
    let mut tunnel = tokio::io::join(recv, send);
    let _ = tokio::io::copy_bidirectional(&mut tcp, &mut tunnel).await;
}

/// Advertise the tunnel to games on this machine, like "Open to LAN" does.
async fn announce(motd: String, port: u16) {
    let Ok(socket) = lan::announcer().await else {
        return;
    };
    let packet = lan::format(&motd, port);
    loop {
        let _ = socket
            .send_to(packet.as_bytes(), (lan::GROUP, lan::PORT))
            .await;
        tokio::time::sleep(lan::INTERVAL).await;
    }
}
