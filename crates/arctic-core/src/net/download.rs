use std::cell::Cell;
use std::collections::HashSet;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use sha1::{Digest, Sha1};

use crate::error::IoContext;
use crate::{Error, Progress, ProgressInfo, Result};

/// Concurrent connections. Most game files are tiny (assets average ~10 KB),
/// so their throughput is bounded by round trips, not bandwidth; many
/// parallel keep-alive connections hide that latency.
pub const WORKERS: usize = 64;
/// Workers that pull from the large end of the queue. Large files saturate
/// bandwidth while everyone else chews through the small ones in parallel,
/// so total time is roughly max(bytes / bandwidth, files / request rate).
const LARGE_FILE_WORKERS: usize = 8;
const BUF_SIZE: usize = 64 * 1024;
const ATTEMPTS: u32 = 3;
const RETRY_BACKOFF: Duration = Duration::from_millis(400);
/// Body timeout = base + size / slowest acceptable speed.
const BODY_TIMEOUT_BASE: Duration = Duration::from_secs(15);
const MIN_SPEED_BYTES_PER_SEC: u64 = 100 * 1024;
/// Minimum interval between progress callbacks.
const REPORT_EVERY_MS: u64 = 50;

/// One file to fetch. `sha1`/`size` describe the final file on disk and are
/// verified when present.
#[derive(Debug, Clone)]
pub struct DownloadJob {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    /// Fetch this LZMA stream instead of `url` and decompress on the fly.
    pub lzma: Option<Compressed>,
}

/// An LZMA-compressed alternative source for a file.
#[derive(Debug, Clone)]
pub struct Compressed {
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

impl DownloadJob {
    /// Bytes that actually travel over the network.
    fn transfer_size(&self) -> Option<u64> {
        self.lzma.as_ref().map(|c| c.size).or(self.size)
    }

    /// Cheap "is it already there" check: existence + size. Full hashing of
    /// thousands of assets on every launch would make launches slow.
    fn is_satisfied(&self) -> bool {
        match (fs::metadata(&self.dest), self.size) {
            (Ok(meta), Some(size)) => meta.is_file() && meta.len() == size,
            (Ok(meta), None) => meta.is_file(),
            (Err(_), _) => false,
        }
    }
}

/// Aggregated, rate-limited progress reporting shared by all workers.
struct Reporter<'a> {
    stage: &'a str,
    progress: Progress<'a>,
    start: Instant,
    last_ms: AtomicU64,
    files_done: AtomicU64,
    files_total: u64,
    bytes_done: AtomicU64,
    bytes_total: u64,
}

impl Reporter<'_> {
    fn add_bytes(&self, n: u64) {
        self.bytes_done.fetch_add(n, Ordering::Relaxed);
        self.report(false);
    }

    /// Undo the bytes of a failed attempt so a retry doesn't double count.
    fn remove_bytes(&self, n: u64) {
        self.bytes_done.fetch_sub(n, Ordering::Relaxed);
    }

    fn file_done(&self) {
        let done = self.files_done.fetch_add(1, Ordering::Relaxed) + 1;
        self.report(done == self.files_total);
    }

    fn report(&self, force: bool) {
        let now = self.start.elapsed().as_millis() as u64;
        let last = self.last_ms.load(Ordering::Relaxed);
        let due = now >= last + REPORT_EVERY_MS
            && self
                .last_ms
                .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok();
        if force || due {
            (self.progress)(ProgressInfo {
                stage: self.stage,
                done: self.files_done.load(Ordering::Relaxed),
                total: self.files_total,
                bytes_done: self.bytes_done.load(Ordering::Relaxed),
                bytes_total: self.bytes_total,
            });
        }
    }
}

/// Download every job that is not already present, `WORKERS` at a time.
/// Returns the first error encountered (remaining jobs are abandoned).
pub fn download_all(stage: &str, jobs: Vec<DownloadJob>, progress: Progress) -> Result<()> {
    let pending = pending_jobs(jobs);
    let reporter = Reporter {
        stage,
        progress,
        start: Instant::now(),
        last_ms: AtomicU64::new(0),
        files_done: AtomicU64::new(0),
        files_total: pending.len() as u64,
        bytes_done: AtomicU64::new(0),
        bytes_total: pending.iter().filter_map(DownloadJob::transfer_size).sum(),
    };
    reporter.report(true);
    if pending.is_empty() {
        return Ok(());
    }

    // (front, back) of the not-yet-started range of `pending`.
    let queue = Mutex::new((0usize, pending.len()));
    let first_error: Mutex<Option<Error>> = Mutex::new(None);
    std::thread::scope(|scope| {
        for worker in 0..WORKERS.min(pending.len()) {
            let (queue, first_error, pending, reporter) =
                (&queue, &first_error, &pending, &reporter);
            scope.spawn(move || {
                loop {
                    if first_error.lock().map(|e| e.is_some()).unwrap_or(true) {
                        return;
                    }
                    let Some(idx) = take(queue, worker < LARGE_FILE_WORKERS) else {
                        return;
                    };
                    let job = &pending[idx];
                    if let Err(e) = download_with_retry(job, reporter) {
                        if let Ok(mut slot) = first_error.lock() {
                            slot.get_or_insert(e);
                        }
                        return;
                    }
                    reporter.file_done();
                }
            });
        }
    });

    match first_error.into_inner() {
        Ok(Some(e)) => Err(e),
        Ok(None) => Ok(()),
        Err(_) => Err(Error::Other("download worker panicked".into())),
    }
}

/// Next job index from the large (front) or small (back) end.
fn take(queue: &Mutex<(usize, usize)>, large: bool) -> Option<usize> {
    let mut range = queue.lock().ok()?;
    let (front, back) = &mut *range;
    if front >= back {
        return None;
    }
    if large {
        *front += 1;
        Some(*front - 1)
    } else {
        *back -= 1;
        Some(*back)
    }
}

/// Jobs still to fetch, one per destination, largest first. Asset indexes
/// map many names to the same content hash, and two workers writing the same
/// `.part` file would corrupt it. Starting big files first keeps one slow
/// download (e.g. the client jar) from finishing last on its own.
fn pending_jobs(jobs: Vec<DownloadJob>) -> Vec<DownloadJob> {
    let mut seen = HashSet::new();
    let mut pending: Vec<DownloadJob> = jobs
        .into_iter()
        .filter(|j| seen.insert(j.dest.clone()) && !j.is_satisfied())
        .collect();
    pending.sort_by_key(|j| std::cmp::Reverse(j.transfer_size().unwrap_or(0)));
    pending
}

/// Network hiccups (a stalled connection, a reset, a corrupted transfer)
/// shouldn't fail a whole launch: retry each file a few times.
fn download_with_retry(job: &DownloadJob, reporter: &Reporter) -> Result<()> {
    let mut attempt = 1;
    loop {
        let transferred = Cell::new(0);
        match download_one(job, reporter, &transferred) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < ATTEMPTS => {
                log::warn!("retrying {} (attempt {attempt}): {e}", job.url);
                reporter.remove_bytes(transferred.get());
                std::thread::sleep(RETRY_BACKOFF * attempt);
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

pub(crate) fn body_timeout(size: Option<u64>) -> Duration {
    BODY_TIMEOUT_BASE + Duration::from_secs(size.unwrap_or(0) / MIN_SPEED_BYTES_PER_SEC)
}

/// Stream into `<dest>.part` (decompressing if needed), hashing both the
/// transferred and the final bytes, verify, then atomically rename.
/// `transferred` tracks bytes reported so far, for retry accounting.
fn download_one(job: &DownloadJob, reporter: &Reporter, transferred: &Cell<u64>) -> Result<()> {
    if let Some(parent) = job.dest.parent() {
        fs::create_dir_all(parent).at(parent)?;
    }
    let tmp = part_path(&job.dest);
    let url = job.lzma.as_ref().map_or(&job.url, |c| &c.url);
    let mut resp = super::agent()
        .get(url)
        .config()
        .timeout_recv_body(Some(body_timeout(job.transfer_size())))
        .build()
        .call()?;
    let on_bytes = |n: u64| {
        transferred.set(transferred.get() + n);
        reporter.add_bytes(n);
    };
    let mut input = Hashing::new(resp.body_mut().as_reader(), &on_bytes);
    let mut output = Hashing::new(fs::File::create(&tmp).at(&tmp)?, &|_| {});
    let copied = match &job.lzma {
        Some(_) => {
            let mut buffered = BufReader::with_capacity(BUF_SIZE, &mut input);
            lzma_rs::lzma_decompress(&mut buffered, &mut output)
                .map_err(|e| Error::Other(format!("corrupt download {url}: {e}")))
        }
        None => copy(&mut input, &mut output).at(&tmp),
    };
    let result = copied
        .and_then(|()| output.inner.flush().at(&tmp))
        .and_then(|()| verify(job, &input, &output));
    drop(output);
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::rename(&tmp, &job.dest).at(&job.dest)
}

/// Both the transferred stream (if compressed) and the final file must match.
fn verify<R, W>(job: &DownloadJob, input: &Hashing<R>, output: &Hashing<W>) -> Result<()> {
    let compressed_ok = job
        .lzma
        .as_ref()
        .is_none_or(|c| c.size == input.count && c.sha1.eq_ignore_ascii_case(&input.hex_digest()));
    let raw_ok = job.size.is_none_or(|s| s == output.count)
        && job
            .sha1
            .as_deref()
            .is_none_or(|want| want.eq_ignore_ascii_case(&output.hex_digest()));
    if compressed_ok && raw_ok {
        Ok(())
    } else {
        Err(Error::Checksum(job.url.clone()))
    }
}

fn copy(input: &mut impl Read, output: &mut impl Write) -> std::io::Result<()> {
    let mut buf = vec![0u8; BUF_SIZE];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        output.write_all(&buf[..n])?;
    }
}

/// Reader/writer adapter that SHA-1s and counts everything passing through.
struct Hashing<'a, T> {
    inner: T,
    hasher: Sha1,
    count: u64,
    on_bytes: &'a dyn Fn(u64),
}

impl<'a, T> Hashing<'a, T> {
    fn new(inner: T, on_bytes: &'a dyn Fn(u64)) -> Self {
        Self {
            inner,
            hasher: Sha1::new(),
            count: 0,
            on_bytes,
        }
    }

    fn hex_digest(&self) -> String {
        hex::encode(self.hasher.clone().finalize())
    }

    fn record(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
        self.count += bytes.len() as u64;
        (self.on_bytes)(bytes.len() as u64);
    }
}

impl<T: Read> Read for Hashing<'_, T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.record(&buf[..n]);
        Ok(n)
    }
}

impl<T: Write> Write for Hashing<'_, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.record(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// `<dest>.part`. Appends rather than replacing the extension, otherwise
/// e.g. `java.exe` and `java.dll` would race on the same temp file.
fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

/// SHA-1 of a file on disk, lowercase hex.
pub fn sha1_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).at(path)?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; BUF_SIZE];
    loop {
        let n = file.read(&mut buf).at(path)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(dest: &str, size: Option<u64>) -> DownloadJob {
        DownloadJob {
            url: String::new(),
            dest: PathBuf::from(dest),
            sha1: None,
            size,
            lzma: None,
        }
    }

    #[test]
    fn satisfied_requires_matching_size() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("f.bin");
        fs::write(&dest, b"hello").unwrap();
        let j = |size| DownloadJob {
            dest: dest.clone(),
            ..job("", size)
        };
        assert!(j(Some(5)).is_satisfied());
        assert!(!j(Some(6)).is_satisfied());
        assert!(j(None).is_satisfied());
    }

    #[test]
    fn sha1_of_known_content() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        fs::write(&p, b"abc").unwrap();
        assert_eq!(
            sha1_file(&p).unwrap(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn part_paths_do_not_collide_across_extensions() {
        let exe = part_path(Path::new("bin/java.exe"));
        let dll = part_path(Path::new("bin/java.dll"));
        assert_ne!(exe, dll);
        assert_eq!(exe, Path::new("bin/java.exe.part"));
    }

    #[test]
    fn pending_dedupes_and_sorts_largest_first() {
        let pending = pending_jobs(vec![
            job("missing/a", Some(1)),
            job("missing/a", Some(1)),
            job("missing/b", Some(300)),
            job("missing/c", None),
        ]);
        let order: Vec<_> = pending.iter().map(|j| j.dest.clone()).collect();
        assert_eq!(
            order,
            ["missing/b", "missing/a", "missing/c"].map(PathBuf::from)
        );
    }

    #[test]
    fn hashing_adapter_counts_and_hashes() {
        let seen = Cell::new(0);
        let on = |n: u64| seen.set(seen.get() + n);
        let mut r = Hashing::new(std::io::Cursor::new(b"abc".to_vec()), &on);
        let mut sink = Hashing::new(Vec::new(), &|_| {});
        copy(&mut r, &mut sink).unwrap();
        assert_eq!((r.count, sink.count, seen.get()), (3, 3, 3));
        assert_eq!(
            sink.hex_digest(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn lzma_roundtrip_verifies_both_hashes() {
        let raw = b"arctic ".repeat(1000);
        let mut compressed = Vec::new();
        lzma_rs::lzma_compress(&mut std::io::Cursor::new(&raw), &mut compressed).unwrap();
        let mut input = Hashing::new(std::io::Cursor::new(compressed.clone()), &|_| {});
        let mut output = Hashing::new(Vec::new(), &|_| {});
        let mut buffered = BufReader::new(&mut input);
        lzma_rs::lzma_decompress(&mut buffered, &mut output).unwrap();
        drop(buffered);
        let job = DownloadJob {
            sha1: Some(hex::encode(Sha1::digest(&raw))),
            size: Some(raw.len() as u64),
            lzma: Some(Compressed {
                url: String::new(),
                sha1: hex::encode(Sha1::digest(&compressed)),
                size: compressed.len() as u64,
            }),
            ..job("x", None)
        };
        assert!(verify(&job, &input, &output).is_ok());
        let wrong = DownloadJob {
            sha1: Some("00".into()),
            ..job
        };
        assert!(verify(&wrong, &input, &output).is_err());
    }

    #[test]
    fn queue_serves_both_ends_without_overlap() {
        let queue = Mutex::new((0, 4));
        assert_eq!(take(&queue, true), Some(0));
        assert_eq!(take(&queue, false), Some(3));
        assert_eq!(take(&queue, false), Some(2));
        assert_eq!(take(&queue, true), Some(1));
        assert_eq!(take(&queue, true), None);
        assert_eq!(take(&queue, false), None);
    }

    #[test]
    fn body_timeout_scales_with_size() {
        assert_eq!(body_timeout(None), BODY_TIMEOUT_BASE);
        let big = body_timeout(Some(100 * 1024 * 1024));
        assert_eq!(big, BODY_TIMEOUT_BASE + Duration::from_secs(1024));
    }

    #[test]
    fn empty_job_list_reports_once_and_succeeds() {
        let calls = AtomicU64::new(0);
        let cb = |info: ProgressInfo| {
            assert_eq!((info.done, info.total), (0, 0));
            calls.fetch_add(1, Ordering::Relaxed);
        };
        assert!(download_all("x", vec![], &cb).is_ok());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}
