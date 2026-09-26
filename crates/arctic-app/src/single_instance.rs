//! One launcher per data folder. A second start hands its command line to
//! the running launcher (which shows itself, even from the tray) and exits.
//!
//! The running launcher listens on a loopback port recorded in
//! `launcher.port` with a random token; messages without the token are
//! ignored.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const PORT_FILE: &str = "launcher.port";
const CONNECT_TIMEOUT: Duration = Duration::from_millis(400);
/// Passed by the updater's restart: take over from the old process.
pub const REPLACE_FLAG: &str = "--replace";

pub enum Claim {
    /// We are the launcher; other starts' arguments arrive here.
    Primary(Receiver<Vec<String>>),
    /// Another launcher took over; this process should exit.
    Forwarded,
}

type Wake = Box<dyn Fn() + Send + Sync>;
static WAKE: OnceLock<Mutex<Option<Wake>>> = OnceLock::new();

/// Called (on the listener thread) whenever another start is forwarded.
pub fn on_wake(f: impl Fn() + Send + Sync + 'static) {
    let slot = WAKE.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(Box::new(f));
    }
}

fn wake() {
    if let Some(slot) = WAKE.get()
        && let Ok(guard) = slot.lock()
        && let Some(f) = guard.as_ref()
    {
        f();
    }
}

pub fn claim(root: &Path, args: &[String]) -> Claim {
    let port_file = root.join(PORT_FILE);
    let replacing = args.iter().any(|a| a == REPLACE_FLAG);
    if !replacing
        && let Some((port, token)) = read_port_file(&port_file)
        && forward(port, &token, args)
    {
        return Claim::Forwarded;
    }
    let (tx, rx) = mpsc::channel();
    match TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) {
        Ok(listener) => {
            let token = uuid::Uuid::new_v4().simple().to_string();
            let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
            if let Err(e) = std::fs::write(&port_file, format!("{port} {token}")) {
                log::warn!("single instance: {e}");
            }
            std::thread::Builder::new()
                .name("single-instance".into())
                .spawn(move || listen(listener, &token, &tx))
                .ok();
        }
        Err(e) => log::warn!("single instance listener: {e}"),
    }
    Claim::Primary(rx)
}

fn read_port_file(path: &Path) -> Option<(u16, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut parts = text.split_whitespace();
    let port = parts.next()?.parse().ok()?;
    Some((port, parts.next()?.to_owned()))
}

/// Send our arguments to a running launcher; true if it accepted them.
fn forward(port: u16, token: &str, args: &[String]) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let message = serde_json::json!({ "token": token, "args": args });
    if writeln!(stream, "{message}").is_err() {
        return false;
    }
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).is_ok() && reply.trim() == "ok"
}

fn listen(listener: TcpListener, token: &str, tx: &mpsc::Sender<Vec<String>>) {
    for stream in listener.incoming().flatten() {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut line = String::new();
        let mut reader = BufReader::new(&stream);
        if reader.read_line(&mut line).is_err() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if message["token"].as_str() != Some(token) {
            continue;
        }
        let args = message["args"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let _ = (&stream).write_all(b"ok\n");
        let _ = tx.send(args);
        wake();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_start_is_forwarded() {
        let dir = tempfile::tempdir().unwrap();
        let Claim::Primary(rx) = claim(dir.path(), &[]) else {
            panic!("first start should be primary");
        };
        let args = vec!["--tab".to_owned(), "skins".to_owned()];
        assert!(matches!(claim(dir.path(), &args), Claim::Forwarded));
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), args);
    }

    #[test]
    fn stale_port_file_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(PORT_FILE), "1 nope").unwrap();
        assert!(matches!(claim(dir.path(), &[]), Claim::Primary(_)));
    }
}
