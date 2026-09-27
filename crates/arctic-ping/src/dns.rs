//! SRV lookups (`_minecraft._tcp.<host>`): the system resolver on Windows,
//! a single UDP query to the configured nameserver elsewhere.

/// (target host, port) of the preferred record: lowest priority, then
/// highest weight.
pub fn srv(name: &str) -> Option<(String, u16)> {
    let mut records = lookup(name)?;
    records.sort_by_key(|r| (r.priority, std::cmp::Reverse(r.weight)));
    records
        .into_iter()
        .find(|r| !r.target.is_empty() && r.target != ".")
        .map(|r| (r.target, r.port))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    priority: u16,
    weight: u16,
    port: u16,
    target: String,
}

#[cfg(windows)]
fn lookup(name: &str) -> Option<Vec<Record>> {
    use windows_sys::Win32::NetworkManagement::Dns::{
        DNS_QUERY_STANDARD, DNS_RECORDA, DNS_TYPE_SRV, DnsFree, DnsFreeRecordList, DnsQuery_W,
    };

    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // windows-sys types the list as DNS_RECORDA; the layout is the same and
    // DnsQuery_W fills its strings with UTF-16.
    let mut results: *mut DNS_RECORDA = std::ptr::null_mut();
    // SAFETY: `wide` is NUL-terminated; `results` receives a list we free below.
    let status = unsafe {
        DnsQuery_W(
            wide.as_ptr(),
            DNS_TYPE_SRV,
            DNS_QUERY_STANDARD,
            std::ptr::null_mut(),
            &mut results,
            std::ptr::null_mut(),
        )
    };
    if status != 0 || results.is_null() {
        return None;
    }
    let mut records = Vec::new();
    let mut at = results;
    while !at.is_null() {
        // SAFETY: `at` walks the list DnsQuery_W returned, which stays valid
        // until DnsFree; SRV data is read only from SRV records.
        let record = unsafe { &*at };
        if record.wType == DNS_TYPE_SRV {
            let srv = unsafe { record.Data.SRV };
            records.push(Record {
                priority: srv.wPriority,
                weight: srv.wWeight,
                port: srv.wPort,
                target: unsafe { wide_str(srv.pNameTarget.cast::<u16>()) },
            });
        }
        at = record.pNext;
    }
    // SAFETY: frees exactly the list DnsQuery_W allocated.
    unsafe { DnsFree(results.cast(), DnsFreeRecordList) };
    Some(records)
}

/// # Safety
/// `p` is null or a NUL-terminated UTF-16 string.
#[cfg(windows)]
unsafe fn wide_str(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    // SAFETY: the caller promises NUL termination.
    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
}

#[cfg(not(windows))]
fn lookup(name: &str) -> Option<Vec<Record>> {
    use std::net::UdpSocket;
    use std::time::Duration;

    let conf = std::fs::read_to_string("/etc/resolv.conf").ok()?;
    let server = conf.lines().find_map(|l| {
        let mut parts = l.split_whitespace();
        (parts.next() == Some("nameserver"))
            .then(|| parts.next())
            .flatten()
    })?;
    let ip: std::net::IpAddr = server.split('%').next()?.parse().ok()?;
    let socket = UdpSocket::bind(if ip.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }).ok()?;
    socket.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    let id = std::process::id() as u16 ^ 0x5a17;
    socket.send_to(&query(id, name)?, (ip, 53)).ok()?;
    let mut buf = [0u8; 4096];
    let n = socket.recv(&mut buf).ok()?;
    parse_answer(id, &buf[..n])
}

const TYPE_SRV: u16 = 33;

#[cfg_attr(windows, allow(dead_code))]
fn query(id: u16, name: &str) -> Option<Vec<u8>> {
    let mut q = Vec::with_capacity(32 + name.len());
    q.extend_from_slice(&id.to_be_bytes());
    // Recursion desired, one question.
    q.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]);
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() || label.len() > 63 {
            return None;
        }
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&TYPE_SRV.to_be_bytes());
    q.extend_from_slice(&1u16.to_be_bytes());
    Some(q)
}

#[cfg_attr(windows, allow(dead_code))]
fn parse_answer(id: u16, msg: &[u8]) -> Option<Vec<Record>> {
    let u16_at = |i: usize| Some(u16::from_be_bytes([*msg.get(i)?, *msg.get(i + 1)?]));
    if u16_at(0)? != id || msg.get(3)? & 0x0f != 0 {
        return None;
    }
    let questions = u16_at(4)?;
    let answers = u16_at(6)?;
    let mut at = 12;
    for _ in 0..questions {
        at = read_name(msg, at)?.1 + 4;
    }
    let mut records = Vec::new();
    for _ in 0..answers {
        let (_, next) = read_name(msg, at)?;
        let kind = u16_at(next)?;
        let len = u16_at(next + 8)? as usize;
        let data = next + 10;
        if kind == TYPE_SRV {
            records.push(Record {
                priority: u16_at(data)?,
                weight: u16_at(data + 2)?,
                port: u16_at(data + 4)?,
                target: read_name(msg, data + 6)?.0,
            });
        }
        at = data + len;
    }
    Some(records)
}

/// A (possibly compressed) name at `at`, and where the data after it starts.
#[cfg_attr(windows, allow(dead_code))]
fn read_name(msg: &[u8], mut at: usize) -> Option<(String, usize)> {
    let mut labels: Vec<String> = Vec::new();
    let mut end = None;
    for _ in 0..128 {
        let len = *msg.get(at)? as usize;
        if len == 0 {
            return Some((labels.join("."), end.unwrap_or(at + 1)));
        }
        if len & 0xc0 == 0xc0 {
            let target = ((len & 0x3f) << 8) | *msg.get(at + 1)? as usize;
            end.get_or_insert(at + 2);
            at = target;
            continue;
        }
        labels.push(String::from_utf8_lossy(msg.get(at + 1..at + 1 + len)?).into_owned());
        at += 1 + len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_srv_answer_with_compression() {
        let mut msg = query(7, "_minecraft._tcp.example.com").unwrap();
        msg[2] = 0x81;
        msg[3] = 0x80;
        msg[7] = 2;
        for (priority, weight, port) in [(10u16, 5u16, 25577u16), (5, 1, 25590)] {
            msg.extend_from_slice(&[0xc0, 12]);
            msg.extend_from_slice(&TYPE_SRV.to_be_bytes());
            msg.extend_from_slice(&[0, 1, 0, 0, 1, 0]);
            // "mc" + a pointer to "example.com" in the question (offset 28).
            let target = b"\x02mc\xc0\x1c";
            msg.extend_from_slice(&((6 + target.len()) as u16).to_be_bytes());
            for v in [priority, weight, port] {
                msg.extend_from_slice(&v.to_be_bytes());
            }
            msg.extend_from_slice(target);
        }
        let records = parse_answer(7, &msg).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].port, 25590);
        assert_eq!(records[0].target, "mc.example.com");
        assert_eq!(parse_answer(8, &msg), None);
    }
}
