use super::*;
use std::io::BufRead;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::{self, JoinHandle};

struct Server {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    listener: Option<JoinHandle<()>>,
}

impl Server {
    fn start(serve: impl Fn(usize, &str, &mut TcpStream) + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (server_requests, stopped) = (requests.clone(), stop.clone());
        let serve = Arc::new(serve);
        let listener = thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(conn) => conn,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                let (requests, serve) = (server_requests.clone(), serve.clone());
                thread::spawn(move || {
                    // macOS keeps sockets accepted from a non-blocking listener
                    // non-blocking, so a 2 MiB write_all can return WouldBlock.
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(10)))
                        .unwrap();
                    let mut reader = BufReader::new(&mut stream);
                    let mut request = String::new();
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                            break;
                        }
                        request.push_str(&line);
                    }
                    let request = request.to_ascii_lowercase();
                    let index = {
                        let mut all = requests.lock().unwrap();
                        all.push(request.clone());
                        all.len() - 1
                    };
                    serve(index, &request, &mut stream);
                });
            }
        });
        Self {
            base,
            requests,
            stop,
            listener: Some(listener),
        }
    }

    fn job(&self, dir: &Path, data: &[u8]) -> DownloadJob {
        DownloadJob {
            url: format!("{}/client.jar", self.base),
            dest: dir.join("client.jar"),
            sha1: Some(hex::encode(Sha1::digest(data))),
            md5: None,
            size: Some(data.len() as u64),
            lzma: None,
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.listener.take().unwrap().join().unwrap();
    }
}

fn respond(stream: &mut TcpStream, status: u16, len: usize, headers: &str, body: &[u8]) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status} Test\r\nContent-Length: {len}\r\nConnection: close\r\n{headers}\r\n"
    );
    let _ = stream.write_all(body);
}

fn policy(attempts: u32) -> Policy {
    Policy {
        attempts,
        delays: vec![Duration::ZERO],
    }
}

const DATA: &[u8] = b"abcdefghi";
const SUFFIX_RANGE: &str = "Content-Range: bytes 3-8/9\r\n";

fn has_header(request: &str, header: &str) -> bool {
    request.lines().any(|line| line == header)
}

#[test]
fn stalls_on_first_and_later_attempts_resume_within_idle_limit() {
    let server = Server::start(|n, _, stream| match n {
        0 => {
            respond(stream, 200, DATA.len(), "", &DATA[..3]);
            let _ = stream.read(&mut [0]);
        }
        1 => {
            respond(stream, 206, 6, SUFFIX_RANGE, b"");
            let _ = stream.read(&mut [0]);
        }
        _ => respond(stream, 206, 6, SUFFIX_RANGE, &DATA[3..]),
    });
    let dir = tempfile::tempdir().unwrap();
    let job = server.job(dir.path(), DATA);
    let start = Instant::now();
    download_single(&job, None, &policy(3), &|p| {
        if p.done == 1 {
            assert_eq!(p.bytes_done, DATA.len() as u64);
        }
    })
    .unwrap();
    assert!(start.elapsed() < BODY_IDLE * 2 + Duration::from_secs(3));
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(
        requests[1..]
            .iter()
            .all(|r| has_header(r, "range: bytes=3-"))
    );
    assert_eq!(fs::read(&job.dest).unwrap(), DATA);
}

#[test]
fn dropped_stream_resumes_and_ignored_range_restarts() {
    for range_supported in [true, false] {
        let server = Server::start(move |n, _, stream| match n {
            0 => respond(stream, 200, DATA.len(), "", &DATA[..3]),
            _ if range_supported => respond(stream, 206, 6, SUFFIX_RANGE, &DATA[3..]),
            _ => respond(stream, 200, DATA.len(), "", DATA),
        });
        let dir = tempfile::tempdir().unwrap();
        let mut job = server.job(dir.path(), DATA);
        job.md5 = Some(hex::encode(Md5::digest(DATA)));
        download_single(&job, None, &policy(2), &|_| {}).unwrap();
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(has_header(&requests[1], "range: bytes=3-"));
        assert_eq!(fs::read(&job.dest).unwrap(), DATA);
        assert!(!part_path(&job.dest).exists());
    }
}

#[test]
fn failed_checksum_or_invalid_range_discards_the_partial_file() {
    for range in [SUFFIX_RANGE, "Content-Range: bytes 2-7/9\r\n"] {
        let server = Server::start(move |n, _, stream| match n {
            0 => respond(stream, 200, DATA.len(), "", b"bad"),
            1 => respond(stream, 206, 6, range, &DATA[3..]),
            _ => respond(stream, 200, DATA.len(), "", DATA),
        });
        let dir = tempfile::tempdir().unwrap();
        let job = server.job(dir.path(), DATA);
        download_single(&job, None, &policy(3), &|_| {}).unwrap();
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(has_header(&requests[1], "range: bytes=3-"));
        assert!(!requests[2].contains("range:"));
        assert_eq!(fs::read(&job.dest).unwrap(), DATA);
    }
}

#[test]
fn an_unhashed_file_resumes_only_with_a_strong_validator() {
    for strong in [true, false] {
        let server = Server::start(move |n, _, stream| match n {
            0 => respond(
                stream,
                200,
                DATA.len(),
                if strong {
                    "ETag: \"tag\"\r\n"
                } else {
                    "ETag: W/\"tag\"\r\n"
                },
                &DATA[..3],
            ),
            _ if strong => respond(stream, 206, 6, SUFFIX_RANGE, &DATA[3..]),
            _ => respond(stream, 200, DATA.len(), "", DATA),
        });
        let dir = tempfile::tempdir().unwrap();
        let mut job = server.job(dir.path(), DATA);
        job.sha1 = None;
        job.size = None;
        download_single(&job, None, &policy(2), &|_| {}).unwrap();
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(has_header(&requests[1], "range: bytes=3-"), strong);
        assert_eq!(has_header(&requests[1], "if-range: \"tag\""), strong);
        assert_eq!(fs::read(&job.dest).unwrap(), DATA);
    }
}

#[test]
fn compressed_java_stream_resumes_and_both_hashes_are_checked() {
    let data = b"arctic ".repeat(1000);
    let mut bytes = Vec::new();
    lzma_rs::lzma_compress(&mut std::io::Cursor::new(&data), &mut bytes).unwrap();
    let (size, sha1) = (bytes.len(), hex::encode(Sha1::digest(&bytes)));
    let server = Server::start(move |n, _, stream| match n {
        0 => respond(stream, 200, size, "", &bytes[..8]),
        1 => respond(
            stream,
            206,
            size - 8,
            &format!("Content-Range: bytes 8-{}/{size}\r\n", size - 1),
            &bytes[8..],
        ),
        _ => respond(stream, 200, size, "", &bytes),
    });
    let dir = tempfile::tempdir().unwrap();
    let mut job = server.job(dir.path(), &data);
    job.lzma = Some(Compressed {
        url: format!("{}/java.lzma", server.base),
        sha1,
        size: size as u64,
    });
    download_single(&job, None, &policy(2), &|_| {}).unwrap();
    assert!(has_header(
        &server.requests.lock().unwrap()[1],
        "range: bytes=8-"
    ));
    assert_eq!(fs::read(&job.dest).unwrap(), data);

    job.sha1 = Some("0".repeat(40));
    assert!(matches!(
        download_single(&job, None, &policy(1), &|_| {}),
        Err(Error::Checksum(_))
    ));
    job.sha1 = Some(hex::encode(Sha1::digest(&data)));
    job.lzma.as_mut().unwrap().sha1 = "0".repeat(40);
    assert!(matches!(
        download_single(&job, None, &policy(1), &|_| {}),
        Err(Error::Checksum(_))
    ));
    assert_eq!(fs::read(&job.dest).unwrap(), data);
}

#[test]
fn segmented_retry_fetches_only_unfinished_ranges() {
    let size = SEGMENT_MIN_SIZE as usize;
    let data = vec![42; size];
    let failures = AtomicU64::new(0);
    let server = Server::start(move |_, request, stream| {
        let range = request
            .lines()
            .find_map(|line| line.strip_prefix("range: bytes="))
            .unwrap();
        let (start, end) = range.split_once('-').unwrap();
        let (start, end) = (start.parse::<u64>().unwrap(), end.parse::<u64>().unwrap());
        if start == 0 && failures.fetch_add(1, Ordering::Relaxed) < u64::from(ATTEMPTS) {
            respond(stream, 503, 0, "", b"");
            return;
        }
        let len = (end - start + 1) as usize;
        respond(
            stream,
            206,
            len,
            &format!("Content-Range: bytes {start}-{end}/{size}\r\n"),
            &vec![42; len],
        );
    });
    let dir = tempfile::tempdir().unwrap();
    let job = server.job(dir.path(), &data);
    download_single(&job, None, &policy(2), &|p| {
        if p.done == 1 {
            assert_eq!(p.bytes_done, size as u64);
        }
    })
    .unwrap();
    let requests = server.requests.lock().unwrap();
    for (start, end) in segments(&job).unwrap() {
        let range = format!("range: bytes={start}-{}", end - 1);
        let hits = requests.iter().filter(|r| has_header(r, &range)).count();
        if start == 0 {
            assert!(
                hits >= ATTEMPTS as usize + 1,
                "unfinished first range should be retried, got {hits}"
            );
        } else {
            assert_eq!(
                hits, 1,
                "finished range {start} should not be fetched again"
            );
        }
    }
    assert_eq!(fs::read(&job.dest).unwrap(), data);
}

#[test]
fn hashed_batch_tries_supplied_alternates_in_order() {
    let primary = Server::start(|_, _, stream| respond(stream, 404, 0, "", b""));
    let unavailable = Server::start(|_, _, stream| respond(stream, 404, 0, "", b""));
    let mirror = Server::start(|_, _, stream| respond(stream, 200, DATA.len(), "", DATA));
    let dir = tempfile::tempdir().unwrap();
    let job = primary.job(dir.path(), DATA);
    let alternates = DownloadAlternates::from([(
        job.dest.clone(),
        vec![unavailable.base.clone(), mirror.base.clone()],
    )]);
    download_all_with_alternates("test", vec![job.clone()], &alternates, &|_| {}).unwrap();
    assert_eq!(primary.requests.lock().unwrap().len(), 1);
    assert_eq!(unavailable.requests.lock().unwrap().len(), 1);
    assert_eq!(mirror.requests.lock().unwrap().len(), 1);
    assert_eq!(fs::read(&job.dest).unwrap(), DATA);
}

#[test]
fn final_error_includes_file_host_and_reason_without_url_secrets() {
    let server = Server::start(|_, _, stream| respond(stream, 503, 0, "", b""));
    let dir = tempfile::tempdir().unwrap();
    let mut job = server.job(dir.path(), DATA);
    job.url.push_str("?token=secret");
    let err = download_single(&job, None, &policy(1), &|_| {}).unwrap_err();
    assert_eq!(
        err.chain(),
        "network error: could not download client.jar from 127.0.0.1: http status: 503"
    );
}

#[test]
fn mojang_alternates_rewrite_only_verified_host_and_path_pairs() {
    let pairs = [
        (
            "piston-data.mojang.com",
            "launcher.mojang.com",
            "/v1/objects/hash/client.jar",
        ),
        (
            "piston-meta.mojang.com",
            "launchermeta.mojang.com",
            "/v1/packages/hash/1.21.1.json",
        ),
        (
            "launchermeta.mojang.com",
            "piston-meta.mojang.com",
            "/v1/products/java-runtime/hash/all.json",
        ),
    ];
    for (a, b, path) in pairs {
        let (a, b) = (format!("https://{a}{path}"), format!("https://{b}{path}"));
        assert_eq!(mojang_alternate(&a).as_deref(), Some(b.as_str()));
        assert_eq!(mojang_alternate(&b).as_deref(), Some(a.as_str()));
    }
    for url in [
        "http://piston-data.mojang.com/v1/objects/hash/client.jar",
        "https://piston-data.mojang.com.example/v1/objects/hash/client.jar",
        "https://piston-data.mojang.com@other.example/v1/objects/hash/client.jar",
        "https://piston-meta.mojang.com/v1/objects/hash/client.jar",
        "https://resources.download.minecraft.net/ab/abc",
        "https://libraries.minecraft.net/a.jar",
    ] {
        assert_eq!(mojang_alternate(url), None, "{url}");
    }
}
