//! Live check: a fake LAN server, a host sharing it and a guest joining by
//! code, all in one process over the real iroh network.
//! `cargo run -p arctic-share --example tunnel_smoke`

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use arctic_share::{Share, ShareEvent};

fn main() {
    env_logger::init();
    // Echo server standing in for a world opened to LAN.
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for mut s in server.incoming().flatten() {
            std::thread::spawn(move || {
                let mut buf = [0u8; 1024];
                while let Ok(n) = s.read(&mut buf) {
                    if n == 0 || s.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
            });
        }
    });

    let started = Instant::now();
    let (htx, hrx) = mpsc::channel();
    let host = Share::new(move |_, e| {
        println!("[host  {:>5.1}s] {e:?}", started.elapsed().as_secs_f32());
        let _ = htx.send(e);
    })
    .unwrap();
    host.host(arctic_share_key(), Some(port));
    let code = loop {
        if let ShareEvent::HostReady { code } = hrx.recv_timeout(Duration::from_secs(30)).unwrap() {
            break code;
        }
    };

    let (gtx, grx) = mpsc::channel();
    let guest = Share::new(move |_, e| {
        println!("[guest {:>5.1}s] {e:?}", started.elapsed().as_secs_f32());
        let _ = gtx.send(e);
    })
    .unwrap();
    guest.join(&code).unwrap();
    let local = loop {
        match grx.recv_timeout(Duration::from_secs(60)).unwrap() {
            ShareEvent::Joined { port, .. } => break port,
            ShareEvent::Stopped { error } => panic!("join failed: {error:?}"),
            _ => {}
        }
    };

    let mut tcp = TcpStream::connect(("127.0.0.1", local)).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    let payload = vec![7u8; 200_000];
    let t = Instant::now();
    tcp.write_all(&payload).unwrap();
    let mut back = vec![0u8; payload.len()];
    tcp.read_exact(&mut back).unwrap();
    assert_eq!(back, payload);
    println!(
        "echoed {} KB through the tunnel in {:.2}s",
        payload.len() / 1000,
        t.elapsed().as_secs_f32()
    );
    // The guest must announce the world to games on this PC.
    let heard = hear_announcement(local);
    println!("guest announcement heard by a local listener: {heard}");
    assert!(heard);
    guest.stop();
    host.stop();
    std::thread::sleep(Duration::from_millis(500));

    // Host with detection: announce like Minecraft's "Open to LAN" does.
    let (dtx, drx) = mpsc::channel();
    let detecting = Share::new(move |_, e| {
        println!("[detect {:>5.1}s] {e:?}", started.elapsed().as_secs_f32());
        let _ = dtx.send(e);
    })
    .unwrap();
    detecting.host(arctic_share_key(), None);
    std::thread::spawn(move || {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
        let packet = format!("[MOTD]Smoke world[/MOTD][AD]{port}[/AD]");
        for _ in 0..8 {
            let _ = socket.send_to(packet.as_bytes(), "224.0.2.60:4445");
            std::thread::sleep(Duration::from_millis(1500));
        }
    });
    let detected = loop {
        match drx.recv_timeout(Duration::from_secs(15)) {
            Ok(ShareEvent::HostWorld(Some(w))) => break w,
            Ok(_) => {}
            Err(_) => panic!("LAN world not detected"),
        }
    };
    assert_eq!(detected.port, port);
    detecting.stop();
    std::thread::sleep(Duration::from_millis(500));
    println!("OK");
}

fn hear_announcement(port: u16) -> bool {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let socket = arctic_share::lan::listener().unwrap();
        let mut buf = [0u8; 512];
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while let Ok(Ok((n, _))) =
            tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await
        {
            if arctic_share::lan::parse(&String::from_utf8_lossy(&buf[..n]))
                .is_some_and(|w| w.port == port)
            {
                return true;
            }
        }
        false
    })
}

fn arctic_share_key() -> [u8; 32] {
    let path = std::env::temp_dir().join("arctic-share-smoke.key");
    arctic_share::load_or_create_key(&path).unwrap()
}
