//! Minecraft's server list ping: handshake, status request, then a ping
//! for the round-trip time; and a login probe. Through the proxy when one
//! is given (and the server isn't local), like the Arctic Client connects.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::address::Address;
use crate::{Error, Result, Socks};

const TIMEOUT: Duration = Duration::from_secs(5);
/// Protocol number sent in the handshake; servers answer status for any.
const STATUS_PROTOCOL: i32 = 772;
/// Largest status response accepted (favicons make them tens of KB).
const MAX_PACKET: usize = 2 * 1024 * 1024;

/// A fresh status JSON and round trip for `address`.
pub fn status_json(address: &Address, proxy: Option<Socks>) -> Result<(String, u32)> {
    let mut stream = connect(address, proxy)?;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(TIMEOUT)))
        .map_err(net_error)?;
    let mut handshake = Vec::new();
    var_int(&mut handshake, 0);
    var_int(&mut handshake, STATUS_PROTOCOL);
    string(&mut handshake, &address.host);
    handshake.extend_from_slice(&address.port.to_be_bytes());
    var_int(&mut handshake, 1);
    send(&mut stream, &handshake)?;
    send(&mut stream, &[0])?;
    let response = receive(&mut stream)?;
    let mut r = response.as_slice();
    if read_var_int(&mut r)? != 0 {
        return Err(Error("the server sent something unexpected".into()));
    }
    let len = usize::try_from(read_var_int(&mut r)?).unwrap_or(usize::MAX);
    let json = r
        .get(..len)
        .ok_or_else(|| Error("the server's answer was cut short".into()))?;
    let json = String::from_utf8_lossy(json).into_owned();

    let mut ping = vec![1];
    let started = Instant::now();
    ping.extend_from_slice(&0x4172_6374_6963_u64.to_be_bytes());
    send(&mut stream, &ping)?;
    let rtt = match receive(&mut stream) {
        Ok(_) => started.elapsed().as_millis().min(u32::MAX as u128) as u32,
        // Some servers close after the status; the answer still counts.
        Err(_) => 0,
    };
    Ok((json, rtt))
}

/// How a server answers a login: premium servers (online mode) ask for
/// encryption to check the account with Mojang; cracked ones let anyone in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Login {
    Premium,
    Cracked,
    /// Turned away before deciding (whitelist, version, anti-bot): why.
    Refused(String),
}

/// Name used for the login probe (never joins: we hang up at the answer).
const PROBE_NAME: &str = "ArcticCheck";
const PROBE_UUID: [u8; 16] = *b"Arctic@\x00\x80\x00Check!";

/// Start a login as the server's own `protocol` (from its status) and see
/// whether it asks for encryption. A cracked server would let the probe in,
/// so the connection is closed at its first answer.
pub fn login_check(address: &Address, protocol: i32) -> Result<Login> {
    let mut stream = connect(address, None)?;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(TIMEOUT)))
        .map_err(net_error)?;
    let mut handshake = Vec::new();
    var_int(&mut handshake, 0);
    var_int(&mut handshake, protocol);
    string(&mut handshake, &address.host);
    handshake.extend_from_slice(&address.port.to_be_bytes());
    var_int(&mut handshake, 2);
    send(&mut stream, &handshake)?;
    send(&mut stream, &login_start(protocol))?;
    let answer = receive(&mut stream)?;
    let _ = stream.shutdown(std::net::Shutdown::Both);
    let mut r = answer.as_slice();
    Ok(match read_var_int(&mut r)? {
        1 => Login::Premium,
        2 | 3 => Login::Cracked,
        0 => {
            let len = usize::try_from(read_var_int(&mut r)?).unwrap_or(0);
            let text = r
                .get(..len)
                .map(String::from_utf8_lossy)
                .unwrap_or_default();
            let reason = serde_json::from_str(&text)
                .map(|v| crate::tidy_motd(&crate::component_text(&v)))
                .unwrap_or_else(|_| crate::tidy_motd(&text));
            Login::Refused(reason)
        }
        n => Login::Refused(format!("unexpected answer ({n})")),
    })
}

/// Login Start as each protocol expects it.
fn login_start(protocol: i32) -> Vec<u8> {
    let mut p = Vec::new();
    var_int(&mut p, 0);
    string(&mut p, PROBE_NAME);
    match protocol {
        // 1.20.2+: the UUID, always.
        764.. => p.extend_from_slice(&PROBE_UUID),
        // 1.19.3 – 1.20.1: an optional UUID.
        761..=763 => {
            p.push(1);
            p.extend_from_slice(&PROBE_UUID);
        }
        // 1.19.1/1.19.2: no signature data, no UUID.
        760 => p.extend_from_slice(&[0, 0]),
        // 1.19: no signature data.
        759 => p.push(0),
        _ => {}
    }
    p
}

fn connect(address: &Address, proxy: Option<Socks>) -> Result<TcpStream> {
    match proxy.filter(|_| !address.is_local()) {
        // The proxy resolves names; no local DNS or SRV lookups.
        Some(proxy) => socks5(proxy, address),
        None => {
            let target = address.redirected();
            let addrs = (target.host.as_str(), target.port)
                .to_socket_addrs()
                .map_err(|_| Error("couldn't find that server (check the address)".into()))?;
            let mut last = None;
            for addr in addrs {
                match TcpStream::connect_timeout(&addr, TIMEOUT) {
                    Ok(s) => return Ok(s),
                    Err(e) => last = Some(e),
                }
            }
            Err(last.map_or_else(
                || Error("couldn't find that server (check the address)".into()),
                net_error,
            ))
        }
    }
}

fn socks5(proxy: Socks, target: &Address) -> Result<TcpStream> {
    let proxy_err = |what: &str| Error(format!("the proxy {what}"));
    let addr = (proxy.host.trim(), proxy.port)
        .to_socket_addrs()
        .map_err(|_| proxy_err("address couldn't be found"))?
        .next()
        .ok_or_else(|| proxy_err("address couldn't be found"))?;
    let mut s =
        TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|_| proxy_err("isn't answering"))?;
    s.set_read_timeout(Some(TIMEOUT)).map_err(net_error)?;
    let with_login = !proxy.username.is_empty();
    s.write_all(if with_login {
        &[5, 2, 0, 2]
    } else {
        &[5, 1, 0]
    })
    .map_err(net_error)?;
    let mut reply = [0u8; 2];
    s.read_exact(&mut reply).map_err(net_error)?;
    match reply {
        [5, 0] => {}
        [5, 2] if with_login => {
            let (user, pass) = (proxy.username.as_bytes(), proxy.password.as_bytes());
            let mut auth = vec![1, user.len().min(255) as u8];
            auth.extend_from_slice(&user[..user.len().min(255)]);
            auth.push(pass.len().min(255) as u8);
            auth.extend_from_slice(&pass[..pass.len().min(255)]);
            s.write_all(&auth).map_err(net_error)?;
            s.read_exact(&mut reply).map_err(net_error)?;
            if reply[1] != 0 {
                return Err(proxy_err("refused the username or password"));
            }
        }
        _ => return Err(proxy_err("wants a login method Arctic doesn't support")),
    }
    let mut req = vec![5, 1, 0];
    match target.host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            req.push(1);
            req.extend_from_slice(&ip.octets());
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            req.push(4);
            req.extend_from_slice(&ip.octets());
        }
        Err(_) => {
            let host = target.host.as_bytes();
            if host.len() > 255 {
                return Err(Error("that server address is too long".into()));
            }
            req.push(3);
            req.push(host.len() as u8);
            req.extend_from_slice(host);
        }
    }
    req.extend_from_slice(&target.port.to_be_bytes());
    s.write_all(&req).map_err(net_error)?;
    let mut head = [0u8; 4];
    s.read_exact(&mut head).map_err(net_error)?;
    if head[1] != 0 {
        return Err(Error(match head[1] {
            3 | 4 => "the proxy couldn't reach that server".into(),
            5 => "that server refused the connection".into(),
            n => format!("the proxy couldn't connect (error {n})"),
        }));
    }
    let bound = match head[3] {
        1 => 4,
        4 => 16,
        3 => {
            let mut n = [0u8; 1];
            s.read_exact(&mut n).map_err(net_error)?;
            n[0] as usize
        }
        _ => return Err(proxy_err("sent something unexpected")),
    };
    let mut rest = vec![0u8; bound + 2];
    s.read_exact(&mut rest).map_err(net_error)?;
    Ok(s)
}

fn net_error(e: std::io::Error) -> Error {
    use std::io::ErrorKind::*;
    Error(
        match e.kind() {
            TimedOut | WouldBlock => "no answer (timed out)",
            ConnectionRefused => "the server refused the connection",
            ConnectionReset | ConnectionAborted | UnexpectedEof => "the server hung up",
            _ => return Error(format!("couldn't connect: {e}")),
        }
        .into(),
    )
}

fn var_int(out: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        if v & !0x7f == 0 {
            out.push(v as u8);
            return;
        }
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
}

fn string(out: &mut Vec<u8>, s: &str) {
    var_int(out, s.len() as i32);
    out.extend_from_slice(s.as_bytes());
}

fn read_var_int(r: &mut impl Read) -> Result<i32> {
    let mut value = 0u32;
    for i in 0..5 {
        let mut b = [0u8; 1];
        r.read_exact(&mut b).map_err(net_error)?;
        value |= u32::from(b[0] & 0x7f) << (7 * i);
        if b[0] & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(Error("the server sent something unexpected".into()))
}

fn send(s: &mut TcpStream, packet: &[u8]) -> Result<()> {
    let mut framed = Vec::with_capacity(packet.len() + 5);
    var_int(&mut framed, packet.len() as i32);
    framed.extend_from_slice(packet);
    s.write_all(&framed).map_err(net_error)
}

fn receive(s: &mut TcpStream) -> Result<Vec<u8>> {
    let len = usize::try_from(read_var_int(s)?).unwrap_or(usize::MAX);
    if len == 0 || len > MAX_PACKET {
        return Err(Error("the server sent something unexpected".into()));
    }
    let mut buf = vec![0u8; len];
    s.read_exact(&mut buf).map_err(net_error)?;
    Ok(buf)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn var_ints_round_trip() {
        for v in [0, 1, 127, 128, 25565, 2_097_151, i32::MAX, -1] {
            let mut out = Vec::new();
            var_int(&mut out, v);
            assert_eq!(read_var_int(&mut out.as_slice()).unwrap(), v);
        }
    }

    /// A one-shot fake server answering status with `json`.
    pub(crate) fn fake_server(json: &'static str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let handshake = receive(&mut s).unwrap();
            assert_eq!(handshake[0], 0);
            assert_eq!(*handshake.last().unwrap(), 1);
            assert_eq!(receive(&mut s).unwrap(), vec![0]);
            let mut status = vec![0];
            string(&mut status, json);
            send(&mut s, &status).unwrap();
            let ping = receive(&mut s).unwrap();
            send(&mut s, &ping).unwrap();
        });
        port
    }

    #[test]
    fn login_probe_reads_the_answer() {
        for (answer, expected) in [
            (vec![1u8, 0], Login::Premium),
            (vec![2u8], Login::Cracked),
            (vec![3u8, 0], Login::Cracked),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            std::thread::spawn(move || {
                let (mut s, _) = listener.accept().unwrap();
                let handshake = receive(&mut s).unwrap();
                assert_eq!(*handshake.last().unwrap(), 2);
                let start = receive(&mut s).unwrap();
                assert_eq!(start.len(), 1 + 1 + PROBE_NAME.len() + 16);
                send(&mut s, &answer).unwrap();
            });
            let address = Address::parse(&format!("127.0.0.1:{port}")).unwrap();
            assert_eq!(login_check(&address, 772).unwrap(), expected);
        }
    }

    #[test]
    fn login_start_fits_each_protocol() {
        let base = 1 + 1 + PROBE_NAME.len();
        assert_eq!(login_start(47).len(), base);
        assert_eq!(login_start(759).len(), base + 1);
        assert_eq!(login_start(760).len(), base + 2);
        assert_eq!(login_start(763).len(), base + 17);
        assert_eq!(login_start(772).len(), base + 16);
    }

    #[test]
    fn pings_a_server() {
        let port = fake_server(r#"{"version":{"name":"1.21.8","protocol":772}}"#);
        let address = Address::parse(&format!("127.0.0.1:{port}")).unwrap();
        let (json, _) = status_json(&address, None).unwrap();
        assert!(json.contains("1.21.8"));
    }
}
