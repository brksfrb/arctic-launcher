use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use md5::Md5;
use sha1::{Digest, Sha1};
use ureq::ResponseExt;
use ureq::http::{HeaderValue, Response, Uri};

use crate::error::IoContext;
use crate::{Error, Progress, ProgressInfo, Result};

/// Concurrent connections. Most game files are tiny (assets average ~10 KB),
/// so their throughput is bounded by round trips, not bandwidth; many
/// parallel keep-alive connections hide that latency.
pub const WORKERS: usize = 64;
pub(crate) type DownloadAlternates = HashMap<PathBuf, Vec<String>>;
/// Downloads at a time in gentle mode (a computer that freezes under load).
const GENTLE_WORKERS: usize = 6;
const GENTLE_CONNECTIONS: usize = 2;

static GENTLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Download gently (few connections, for a PC that can't take a burst) from now on.
pub fn set_gentle(on: bool) {
    GENTLE.store(on, Ordering::Relaxed);
}

fn workers() -> usize {
    if GENTLE.load(Ordering::Relaxed) {
        GENTLE_WORKERS
    } else {
        WORKERS
    }
}

fn connections() -> usize {
    if GENTLE.load(Ordering::Relaxed) {
        GENTLE_CONNECTIONS
    } else {
        CONNECTIONS
    }
}
/// Workers that pull from the large end of the queue. Large files saturate
/// bandwidth while everyone else chews through the small ones in parallel,
/// so total time is roughly max(bytes / bandwidth, files / request rate).
const LARGE_FILE_WORKERS: usize = 8;
const BUF_SIZE: usize = 64 * 1024;
const ATTEMPTS: u32 = 5;
const RETRY_BACKOFF: Duration = Duration::from_millis(400);
/// Loader hosts (Forge/NeoForge maven and the Forge mirror) can take 15 s just to accept a
/// connection, so they get a longer connect timeout and fewer, slower retries.
const LOADER_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const LOADER_BACKOFF: [Duration; 2] = [Duration::from_secs(2), Duration::from_secs(5)];
const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/";
/// Mirror of the Forge maven, tried only after the primary failed.
const FORGE_MIRROR: &str = "https://bmclapi2.bangbang93.com/maven/";

fn is_loader_host(url: &str) -> bool {
    let host = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = host.split('/').next().unwrap_or_default();
    ["minecraftforge.net", "neoforged.net", "bangbang93.com"]
        .iter()
        .any(|h| host.ends_with(h))
}

/// Connect timeout for a request to `url`.
pub(crate) fn connect_timeout(url: &str) -> Option<Duration> {
    Some(if is_loader_host(url) {
        LOADER_CONNECT_TIMEOUT
    } else {
        super::CONNECT_TIMEOUT
    })
}

/// How often and how long to wait between attempts at one URL.
pub(crate) struct Policy {
    pub(crate) attempts: u32,
    pub(crate) delays: Vec<Duration>,
}

impl Policy {
    pub(crate) fn for_url(url: &str) -> Self {
        if is_loader_host(url) {
            Self {
                attempts: 3,
                delays: LOADER_BACKOFF.to_vec(),
            }
        } else {
            Self {
                attempts: ATTEMPTS,
                delays: (1..ATTEMPTS).map(|n| RETRY_BACKOFF * n).collect(),
            }
        }
    }
}

/// A missing file stays missing; retrying only delays the error.
pub(crate) fn is_not_found(e: &Error) -> bool {
    matches!(e, Error::Http(m) if m.ends_with("http status: 404"))
}

/// The mirror URL for a Forge maven file, if `job` may be fetched from there. Nothing from the
/// mirror is accepted unless its bytes are checked against a hash that did not come from the
/// mirror, so a job without one never goes there (the installer jar included).
fn mirror_for(job: &DownloadJob) -> Option<String> {
    if job.sha1.is_none() && job.md5.is_none() {
        return None;
    }
    forge_mirror_url(&job.url)
}

/// The existing Forge mirror's URL, before checking whether the job has a hash.
pub(crate) fn forge_mirror_url(url: &str) -> Option<String> {
    let path = url.strip_prefix(FORGE_MAVEN)?;
    Some(format!("{FORGE_MIRROR}{path}"))
}

/// Mojang publishes identical content-addressed paths on these pairs of hosts.
pub(crate) fn mojang_alternate(url: &str) -> Option<String> {
    let (host, path) = url.strip_prefix("https://")?.split_once('/')?;
    let alternate = match host {
        "piston-meta.mojang.com" | "launchermeta.mojang.com"
            if path.starts_with("v1/packages/")
                || path.starts_with("v1/products/java-runtime/") =>
        {
            if host == "piston-meta.mojang.com" {
                "launchermeta.mojang.com"
            } else {
                "piston-meta.mojang.com"
            }
        }
        "piston-data.mojang.com" if path.starts_with("v1/objects/") => "launcher.mojang.com",
        "launcher.mojang.com" if path.starts_with("v1/objects/") => "piston-data.mojang.com",
        _ => return None,
    };
    Some(format!("https://{alternate}/{path}"))
}

/// Body timeout = base + size / slowest acceptable speed. The speed is per file, and up to
/// [`WORKERS`] files share the connection: on a modest line each gets a few dozen KB/s, so the
/// bar is low enough that only a connection that has really stalled gives up (a 100 KB/s bar
/// failed 1 MB libraries on ordinary home connections: "timeout: receive body").
const BODY_TIMEOUT_BASE: Duration = Duration::from_secs(30);
const MIN_SPEED_BYTES_PER_SEC: u64 = 8 * 1024;
/// Files whose size isn't known up front (installers, Maven libraries without a size): the whole
/// body may take this long. A flat 30 s failed on slow lines, five times from byte 0.
const BODY_TIMEOUT_UNKNOWN_SIZE: Duration = Duration::from_secs(300);
/// Minimum interval between progress callbacks.
const REPORT_EVERY_MS: u64 = 50;
/// Files at least this big are fetched in parallel byte ranges, so one slow
/// server connection doesn't cap the speed of a big jar.
const SEGMENT_MIN_SIZE: u64 = 8 * 1024 * 1024;
/// A big file's chunks, and the connections fetching them in turn.
const CHUNK_SIZE: u64 = 2 * 1024 * 1024;
const CONNECTIONS: usize = 6;
/// A chunk's slowest acceptable speed (its own connection; a 1 Mbit/s line shared by
/// [`CONNECTIONS`] gives each ~20 KB/s).
const CHUNK_MIN_SPEED: u64 = 12 * 1024;
/// Limit time without body bytes independently of the overall transfer budget.
const BODY_IDLE: Duration = if cfg!(test) {
    Duration::from_secs(2)
} else {
    Duration::from_secs(12)
};
const TAIL_MIN_SPEED: u64 = 128 * 1024;
const FAST_BATCH_SPEED: u64 = 1024 * 1024;
/// Check for an idle or crawling transfer after this observation window.
const CHUNK_STALL: Duration = if cfg!(test) {
    Duration::from_secs(1)
} else {
    Duration::from_secs(10)
};

/// One file to fetch. `sha1`/`size` describe the final file on disk and are
/// verified when present.
#[derive(Debug, Clone)]
pub struct DownloadJob {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    /// MD5 of the final file, for the one source that publishes only that (Forge's meta.json).
    /// Jobs that require MD5 use the single-stream path.
    pub md5: Option<String>,
    pub size: Option<u64>,
    /// Fetch and verify this LZMA stream instead of `url`, then decompress it.
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
    fn source(&self) -> &str {
        self.lzma.as_ref().map_or(&self.url, |c| &c.url)
    }

    fn with_source(&self, url: String) -> Self {
        let mut job = self.clone();
        if let Some(compressed) = &mut job.lzma {
            compressed.url = url;
        } else {
            job.url = url;
        }
        job
    }

    fn has_hash(&self) -> bool {
        self.sha1.is_some() || self.md5.is_some() || self.lzma.is_some()
    }

    fn file_name(&self) -> std::borrow::Cow<'_, str> {
        self.dest.file_name().unwrap_or_default().to_string_lossy()
    }

    /// Bytes that actually travel over the network.
    fn transfer_size(&self) -> Option<u64> {
        self.lzma.as_ref().map(|c| c.size).or(self.size)
    }

    /// Check existence and size, plus the ZIP boundaries for JARs. Keep the
    /// thousands of assets on the metadata-only path.
    pub(crate) fn is_satisfied(&self) -> bool {
        fs::metadata(&self.dest)
            .is_ok_and(|meta| meta.is_file() && self.size.is_none_or(|size| meta.len() == size))
            && (!self
                .dest
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("jar"))
                || jar_has_zip_headers(&self.dest).unwrap_or(false))
    }
}

fn jar_has_zip_headers(path: &Path) -> std::io::Result<bool> {
    const END_RECORD_LEN: usize = 22;
    let mut file = fs::File::open(path)?;
    let mut start = [0; 4];
    let mut end = [0; END_RECORD_LEN];
    file.read_exact(&mut start)?;
    file.seek(SeekFrom::End(-(END_RECORD_LEN as i64)))?;
    file.read_exact(&mut end)?;
    if start == *b"PK\x03\x04" && end.starts_with(b"PK\x05\x06") && end[20..] == [0, 0] {
        return Ok(true);
    }

    // ZIP comments and other valid layouts can move these records. Let the
    // ZIP reader handle them and check that the first entry has a local header.
    Ok(zip::ZipArchive::new(BufReader::new(file))
        .is_ok_and(|mut jar| jar.is_empty() || jar.by_index_raw(0).is_ok()))
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
    all_sizes_known: bool,
    fast_batch: std::sync::atomic::AtomicBool,
}

impl Reporter<'_> {
    fn add_bytes(&self, n: u64) {
        self.bytes_done.fetch_add(n, Ordering::Relaxed);
        self.report(false);
    }

    /// Undo the bytes of a failed attempt so a retry doesn't double count.
    fn remove_bytes(&self, n: u64) {
        let _ = self
            .bytes_done
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |done| {
                Some(done.saturating_sub(n))
            });
    }

    fn fast_tail(&self) -> bool {
        self.files_done.load(Ordering::Relaxed) + 1 == self.files_total
            && self.fast_batch.load(Ordering::Relaxed)
    }

    fn file_done(&self) {
        if fast_transfer(self.bytes_done.load(Ordering::Relaxed), self.start) {
            self.fast_batch.store(true, Ordering::Relaxed);
        }
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
            let bytes_total = if self.all_sizes_known {
                self.bytes_total
            } else {
                0
            };
            (self.progress)(ProgressInfo {
                stage: self.stage,
                done: self.files_done.load(Ordering::Relaxed),
                total: self.files_total,
                bytes_done: self.bytes_done.load(Ordering::Relaxed).min(bytes_total),
                bytes_total,
            });
        }
    }
}

/// Admission limits for one batch. Already admitted requests can drain after a
/// reduction, while new requests (including retries) wait for the lower limit.
struct TransferLimits {
    ceiling: usize,
    state: Mutex<TransferState>,
    available: Condvar,
}

struct TransferState {
    limit: usize,
    active: usize,
    interruptions: u64,
    recent_interruptions: u64,
    successes: usize,
    hedging: bool,
    stopped: bool,
}

const SUCCESSES_TO_RECOVER: usize = 8;

impl TransferLimits {
    fn new(ceiling: usize) -> Self {
        Self {
            ceiling,
            state: Mutex::new(TransferState {
                limit: ceiling,
                active: 0,
                interruptions: 0,
                recent_interruptions: 0,
                successes: 0,
                hedging: false,
                stopped: false,
            }),
            available: Condvar::new(),
        }
    }

    fn enter(&self) -> Result<TransferSlot<'_>> {
        let state = self.state.lock().unwrap();
        let mut state = self
            .available
            .wait_while(state, |s| !s.stopped && s.active >= s.limit)
            .unwrap();
        if state.stopped {
            return Err(Error::Other("download batch stopped".into()));
        }
        state.active += 1;
        Ok(TransferSlot {
            limits: self,
            interruptions: state.interruptions,
        })
    }

    fn stop(&self) {
        self.state.lock().unwrap().stopped = true;
        self.available.notify_all();
    }

    fn interrupted(&self) {
        let mut state = self.state.lock().unwrap();
        state.interruptions += 1;
        state.recent_interruptions += 1;
        state.successes = 0;
        if state.recent_interruptions >= 2 {
            let limit = (state.limit / 2).max(1);
            if limit != state.limit {
                log::info!("download interruptions reduced concurrency to {limit}");
                state.limit = limit;
            }
        }
    }

    fn success(&self, interruptions_at_start: u64) {
        let mut state = self.state.lock().unwrap();
        if state.interruptions == interruptions_at_start {
            state.successes += 1;
            if state.successes == SUCCESSES_TO_RECOVER {
                state.successes = 0;
                state.recent_interruptions = 0;
                if state.limit < self.ceiling {
                    state.limit += 1;
                    log::info!("successful downloads raised concurrency to {}", state.limit);
                    self.available.notify_all();
                }
            }
        }
    }

    fn allows_ranges(&self) -> bool {
        let state = self.state.lock().unwrap();
        !state.stopped && state.limit == self.ceiling && state.recent_interruptions < 2
    }

    fn try_hedge(self: &Arc<Self>) -> Option<HedgeSlot> {
        let mut state = self.state.lock().unwrap();
        if state.hedging
            || state.stopped
            || state.recent_interruptions >= 2
            || state.limit < self.ceiling
        {
            return None;
        }
        state.hedging = true;
        Some(HedgeSlot(self.clone()))
    }

    fn user_error(&self, error: Error) -> Error {
        if matches!(error, Error::DownloadInterrupted(_))
            && self.state.lock().unwrap().interruptions >= 2
        {
            Error::DownloadConnectionDropping
        } else {
            error
        }
    }
}

struct TransferSlot<'a> {
    limits: &'a TransferLimits,
    interruptions: u64,
}

impl Drop for TransferSlot<'_> {
    fn drop(&mut self) {
        self.limits.state.lock().unwrap().active -= 1;
        self.limits.available.notify_all();
    }
}

struct HedgeSlot(Arc<TransferLimits>);

impl Drop for HedgeSlot {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().hedging = false;
    }
}

/// Download every job that is not already present, with up to `WORKERS` active files.
/// Returns the first error encountered (remaining jobs are abandoned).
pub fn download_all(stage: &str, jobs: Vec<DownloadJob>, progress: Progress) -> Result<()> {
    download_all_with_replacements(stage, jobs, Vec::new(), progress)
}

/// Replacements have already been checked by hash and must bypass the size-only check.
pub(crate) fn download_all_with_replacements(
    stage: &str,
    jobs: Vec<DownloadJob>,
    replacements: Vec<DownloadJob>,
    progress: Progress,
) -> Result<()> {
    download_batch(stage, jobs, replacements, &HashMap::new(), progress)
}

pub(crate) fn download_all_with_alternates(
    stage: &str,
    jobs: Vec<DownloadJob>,
    alternates: &DownloadAlternates,
    progress: Progress,
) -> Result<()> {
    download_batch(stage, jobs, Vec::new(), alternates, progress)
}

fn download_batch(
    stage: &str,
    jobs: Vec<DownloadJob>,
    replacements: Vec<DownloadJob>,
    alternates: &DownloadAlternates,
    progress: Progress,
) -> Result<()> {
    let checks_started = Instant::now();
    let count = replacements.len() + jobs.len();
    let pending = pending_jobs(jobs, replacements);
    log::info!(
        "[startup] {stage}: checked {count} files in {} ms, {} downloads needed",
        checks_started.elapsed().as_millis(),
        pending.len()
    );
    let reporter = Reporter {
        stage,
        progress,
        start: Instant::now(),
        last_ms: AtomicU64::new(0),
        files_done: AtomicU64::new(0),
        files_total: pending.len() as u64,
        bytes_done: AtomicU64::new(0),
        bytes_total: pending.iter().filter_map(DownloadJob::transfer_size).sum(),
        all_sizes_known: pending.iter().all(|job| job.transfer_size().is_some()),
        fast_batch: Default::default(),
    };
    reporter.report(true);
    if pending.is_empty() {
        return Ok(());
    }

    if pending.len() <= 16 {
        let names: Vec<_> = pending
            .iter()
            .filter_map(|job| job.dest.file_name())
            .collect();
        log::info!("[startup] {stage}: pending destinations {names:?}");
    }
    log::logger().flush();

    // (front, back) of the not-yet-started range of `pending`.
    let queue = Mutex::new((0usize, pending.len()));
    let first_error: Mutex<Option<Error>> = Mutex::new(None);
    let worker_count = workers().min(pending.len());
    let limits = Arc::new(TransferLimits::new(worker_count));
    std::thread::scope(|scope| {
        let (active, finished) = std::sync::mpsc::channel::<()>();
        let reporter = &reporter;
        scope.spawn(move || {
            while let Err(std::sync::mpsc::RecvTimeoutError::Timeout) =
                finished.recv_timeout(Duration::from_secs(10))
            {
                log::info!(
                    "[startup] {stage}: {} of {} files, {} bytes received",
                    reporter.files_done.load(Ordering::Relaxed),
                    reporter.files_total,
                    reporter.bytes_done.load(Ordering::Relaxed),
                );
                log::logger().flush();
            }
        });
        for worker in 0..worker_count {
            let (queue, first_error, pending, limits) = (&queue, &first_error, &pending, &limits);
            let active = active.clone();
            scope.spawn(move || {
                let _active = active;
                loop {
                    if first_error.lock().map(|e| e.is_some()).unwrap_or(true) {
                        return;
                    }
                    let Some(idx) = take(queue, worker < LARGE_FILE_WORKERS) else {
                        return;
                    };
                    let job = &pending[idx];
                    let urls = alternates.get(&job.dest).into_iter().flatten().cloned();
                    let mirror = mirror_for(job).filter(|_| job.lzma.is_none());
                    let urls = urls.chain(mojang_alternate(job.source())).chain(mirror);
                    if let Err(e) = download_with_alternates(
                        job,
                        reporter,
                        limits,
                        urls,
                        &Policy::for_url(job.source()),
                    ) {
                        if let Ok(mut slot) = first_error.lock() {
                            slot.get_or_insert(e);
                        }
                        limits.stop();
                        return;
                    }
                    reporter.file_done();
                }
            });
        }
        drop(active);
    });

    // What a slow download looked like, for reports ("downloads take forever").
    let secs = reporter.start.elapsed().as_secs_f64();
    let bytes = reporter.bytes_done.load(Ordering::Relaxed);
    log::info!(
        "{stage}: {} of {} files, {:.1} MB in {secs:.1} s ({:.1} MB/s)",
        reporter.files_done.load(Ordering::Relaxed),
        reporter.files_total,
        bytes as f64 / 1e6,
        bytes as f64 / 1e6 / secs.max(0.001),
    );
    log::logger().flush();
    match first_error.into_inner() {
        Ok(Some(e)) => Err(limits.user_error(e)),
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
fn pending_jobs(jobs: Vec<DownloadJob>, mut replacements: Vec<DownloadJob>) -> Vec<DownloadJob> {
    let mut seen = HashSet::new();
    replacements.retain(|job| seen.insert(job.dest.clone()));
    let unique: Vec<_> = jobs
        .into_iter()
        .filter(|j| seen.insert(j.dest.clone()))
        .collect();
    // Bound disk metadata concurrency independently of the much larger network pool.
    // Small queues avoid thread creation altogether.
    let mut pending: Vec<DownloadJob> = if unique.len() < 128 {
        unique.into_iter().filter(|j| !j.is_satisfied()).collect()
    } else {
        std::thread::scope(|scope| {
            let chunk = unique.len().div_ceil(4);
            let handles: Vec<_> = unique
                .chunks(chunk)
                .map(|jobs| {
                    scope.spawn(move || {
                        jobs.iter()
                            .filter(|j| !j.is_satisfied())
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("file check worker panicked"))
                .collect()
        })
    };
    pending.append(&mut replacements);
    pending.sort_by_key(|j| std::cmp::Reverse(j.transfer_size().unwrap_or(0)));
    pending
}

/// Download one file, falling back to `mirror` (only ever used for hash-verified jobs).
pub(crate) fn download_single(
    job: &DownloadJob,
    mirror: Option<String>,
    policy: &Policy,
    progress: Progress,
) -> Result<()> {
    let reporter = Reporter {
        stage: "Downloading installer",
        progress,
        start: Instant::now(),
        last_ms: AtomicU64::new(0),
        files_done: AtomicU64::new(0),
        files_total: 1,
        bytes_done: AtomicU64::new(0),
        bytes_total: job.transfer_size().unwrap_or(0),
        all_sizes_known: job.transfer_size().is_some(),
        fast_batch: Default::default(),
    };
    reporter.report(true);
    let limits = Arc::new(TransferLimits::new(1));
    download_with_alternates(job, &reporter, &limits, mirror, policy)
        .map_err(|e| limits.user_error(e))?;
    reporter.file_done();
    Ok(())
}

/// GET a small text file (a checksum, a metadata document) with the same retries and backoff as
/// a download from that host. A 404 is final and comes back as `Ok(None)`.
pub(crate) fn fetch_text(url: &str, limit: u64, policy: &Policy) -> Result<Option<String>> {
    let mut attempt = 1;
    loop {
        let result = super::agent()
            .get(url)
            .config()
            .timeout_connect(connect_timeout(url))
            .timeout_global(Some(Duration::from_secs(60)))
            .build()
            .call()
            .map_err(request_error)
            .and_then(|mut resp| {
                with_download_idle(|| resp.body_mut().with_config().limit(limit).read_to_string())
                    .map_err(request_error)
            });
        match result {
            Ok(text) => return Ok(Some(text)),
            Err(e) if is_not_found(&e) => return Ok(None),
            Err(e) => {
                let e = metadata_error(url, e);
                log::warn!("download attempt {attempt} failed: {e}");
                if attempt >= policy.attempts {
                    return Err(e);
                }
                let delay = policy.delays.get(attempt as usize - 1);
                std::thread::sleep(delay.or(policy.delays.last()).copied().unwrap_or_default());
                attempt += 1;
            }
        }
    }
}

fn host(url: &str) -> String {
    url.parse::<Uri>()
        .ok()
        .and_then(|uri| uri.host().map(str::to_owned))
        .unwrap_or_else(|| "unknown host".into())
}

pub(super) fn request_error(e: ureq::Error) -> Error {
    // These errors can contain a full URL, including a signed query string.
    match e {
        ureq::Error::BadUri(_) => Error::Http("invalid URL".into()),
        ureq::Error::RequireHttpsOnly(_) => Error::Http("HTTPS required".into()),
        e => e.into(),
    }
}

fn transfer_error(e: ureq::Error) -> Error {
    use std::io::ErrorKind;
    match &e {
        ureq::Error::Timeout(_) | ureq::Error::BodyStalled | ureq::Error::ConnectionFailed => {
            Error::DownloadInterrupted(e.to_string())
        }
        ureq::Error::Io(io)
            if matches!(
                io.kind(),
                ErrorKind::TimedOut
                    | ErrorKind::WouldBlock
                    | ErrorKind::UnexpectedEof
                    | ErrorKind::ConnectionAborted
                    | ErrorKind::ConnectionReset
                    | ErrorKind::BrokenPipe
            ) =>
        {
            Error::DownloadInterrupted(e.to_string())
        }
        _ => request_error(e),
    }
}

pub(super) fn metadata_error(url: &str, error: Error) -> Error {
    let uri = url.parse::<Uri>().ok();
    let file = uri
        .as_ref()
        .and_then(|uri| uri.path().rsplit('/').next())
        .filter(|name| !name.is_empty())
        .unwrap_or("metadata");
    let reason = match error {
        Error::Http(reason) => reason,
        e => e.chain(),
    };
    Error::Http(format!(
        "could not download {file} from {}: {reason}",
        host(url)
    ))
}

pub(super) fn with_download_idle<T>(read: impl FnOnce() -> T) -> T {
    super::download_transport::with_body_idle(BODY_IDLE, read)
}

fn download_error(job: &DownloadJob, error: Error) -> Error {
    let source = || format!("{} from {}", job.file_name(), host(job.source()));
    match error {
        Error::Http(reason) => Error::Http(format!("could not download {}: {reason}", source())),
        Error::DownloadInterrupted(reason) => {
            Error::DownloadInterrupted(format!("could not download {}: {reason}", source()))
        }
        Error::Checksum(_) => Error::Checksum(source()),
        other => other,
    }
}

/// Retry the primary, then the supplied alternates in order. Prefer a transfer
/// interruption over the primary's error if all sources fail.
fn download_with_alternates(
    job: &DownloadJob,
    reporter: &Reporter,
    limits: &Arc<TransferLimits>,
    alternates: impl IntoIterator<Item = String>,
    policy: &Policy,
) -> Result<()> {
    let mut segmented = SegmentedPart::new(job);
    if segmented.is_some() && job.has_hash() && partial_len(&part_path(&job.dest))? > 0 {
        segmented = None;
    }
    let Err(mut error) = retry_job(job, reporter, limits, policy, &mut segmented) else {
        return Ok(());
    };
    if job.has_hash() {
        let mut tried = HashSet::from([job.source().to_owned()]);
        for url in alternates {
            if !tried.insert(url.clone()) {
                continue;
            }
            log::warn!("trying {} from alternate {}", job.file_name(), host(&url));
            let alternate = job.with_source(url);
            match retry_job(&alternate, reporter, limits, policy, &mut segmented) {
                Ok(()) => return Ok(()),
                Err(e @ Error::DownloadInterrupted(_))
                    if !matches!(error, Error::DownloadInterrupted(_)) =>
                {
                    error = e;
                }
                Err(_) => {}
            }
        }
    }
    // The preallocated file needs the in-memory range map. Never leave it where
    // a future single-stream download could mistake gaps for downloaded bytes.
    if segmented.is_some_and(|part| part.initialized) {
        let tmp = part_path(&job.dest);
        fs::remove_file(&tmp).at(&tmp)?;
    }
    Err(error)
}

fn retry_job(
    job: &DownloadJob,
    reporter: &Reporter,
    limits: &Arc<TransferLimits>,
    policy: &Policy,
    segmented: &mut Option<SegmentedPart>,
) -> Result<()> {
    let mut attempt = 1;
    let mut validator = None;
    let mut interruption = None;
    loop {
        let slot = limits.enter()?;
        if !limits.allows_ranges() {
            switch_to_stream(job, segmented)?;
        }
        let transferred = Cell::new(0);
        let try_fast_recovery = attempt == 1 && policy.attempts > 1;
        let result = if let Some(part) = segmented {
            match download_segmented(job, reporter, limits, &transferred, part) {
                Err(Segmented::UseStream) => {
                    reporter.remove_bytes(transferred.get());
                    switch_to_stream(job, segmented)?;
                    continue;
                }
                Err(Segmented::NoRanges) => {
                    *segmented = None;
                    reporter.remove_bytes(transferred.replace(0));
                    download_one(
                        job,
                        reporter,
                        &transferred,
                        try_fast_recovery,
                        None,
                        &mut validator,
                    )
                }
                Err(Segmented::Failed(e)) => Err(e),
                Ok(()) => Ok(()),
            }
        } else {
            download_one(
                job,
                reporter,
                &transferred,
                try_fast_recovery,
                None,
                &mut validator,
            )
        };
        let Err(e) = result else {
            limits.success(slot.interruptions);
            return Ok(());
        };
        if segmented.is_none() && matches!(e, Error::DownloadInterrupted(_)) {
            limits.interrupted();
        }
        reporter.remove_bytes(transferred.get());
        let e = download_error(job, e);
        log::warn!(
            "download attempt {attempt} for {} from {} failed: {e}",
            job.file_name(),
            host(job.source())
        );
        if attempt >= policy.attempts || is_not_found(&e) {
            return Err(match e {
                Error::Http(_) => interruption.unwrap_or(e),
                _ => e,
            });
        }
        if matches!(e, Error::DownloadInterrupted(_)) {
            interruption = Some(e);
        }
        drop(slot);
        let delay = policy.delays.get(attempt as usize - 1);
        std::thread::sleep(delay.or(policy.delays.last()).copied().unwrap_or_default());
        attempt += 1;
    }
}

/// Resume a preallocated range file as a stream from its contiguous prefix of
/// completed ranges. The completed file still gets its usual hash check.
fn switch_to_stream(job: &DownloadJob, segmented: &mut Option<SegmentedPart>) -> Result<()> {
    if let Some(part) = segmented.take().filter(|p| p.initialized) {
        let tmp = part_path(&job.dest);
        let prefix = part.remaining.first().map_or(0, |range| range.0);
        if job.has_hash() && prefix > 0 {
            fs::OpenOptions::new()
                .write(true)
                .open(&tmp)
                .at(&tmp)?
                .set_len(prefix)
                .at(&tmp)?;
        } else {
            fs::remove_file(&tmp).at(&tmp)?;
        }
    }
    Ok(())
}

pub(crate) fn body_timeout(size: Option<u64>) -> Duration {
    match size {
        Some(size) => BODY_TIMEOUT_BASE + Duration::from_secs(size / MIN_SPEED_BYTES_PER_SEC),
        None => BODY_TIMEOUT_UNKNOWN_SIZE,
    }
}

/// A strong validator permits resuming an unhashed file during this job, even
/// when the server cannot supply a checksum. Bind it to the final redirect URI.
struct Validator {
    etag: HeaderValue,
    uri: Uri,
}

impl Validator {
    fn from_response(resp: &Response<ureq::Body>) -> Option<Self> {
        let etag = resp.headers().get("etag")?;
        let tag = etag.to_str().ok()?;
        (tag.len() >= 2 && tag.starts_with('"') && tag.ends_with('"')).then(|| Self {
            etag: etag.clone(),
            uri: resp.get_uri().clone(),
        })
    }

    fn matches(&self, resp: &Response<ureq::Body>) -> bool {
        self.uri == *resp.get_uri()
            && resp
                .headers()
                .get("etag")
                .is_none_or(|etag| *etag == self.etag)
    }
}

/// Byte offsets in a Content-Range refer to the encoded representation.
fn identity_body(resp: &Response<ureq::Body>) -> bool {
    resp.headers()
        .get("content-encoding")
        .is_none_or(|encoding| encoding.as_bytes().eq_ignore_ascii_case(b"identity"))
}

fn content_range(resp: &Response<ureq::Body>) -> Option<(u64, u64, Option<u64>)> {
    let value = resp.headers().get("content-range")?.to_str().ok()?;
    let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let (start, end) = (start.parse().ok()?, end.parse().ok()?);
    let total = if total == "*" {
        None
    } else {
        Some(total.parse().ok()?)
    };
    (start <= end && total.is_none_or(|total| end < total)).then_some((start, end, total))
}

fn partial_len(tmp: &Path) -> Result<u64> {
    match fs::metadata(tmp) {
        Ok(meta) => Ok(meta.len()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(Error::io(tmp, e)),
    }
}

/// Stream into `<dest>.part`, verify, then atomically rename.
/// Only a verified hash or an unchanged strong ETag can confirm a resumed file.
fn download_one(
    job: &DownloadJob,
    reporter: &Reporter,
    transferred: &Cell<u64>,
    retry_slow_tail: bool,
    timeout: Option<Duration>,
    validator: &mut Option<Validator>,
) -> Result<()> {
    if let Some(parent) = job.dest.parent() {
        fs::create_dir_all(parent).at(parent)?;
    }
    let raw_part = part_path(&job.dest);
    let tmp = if job.lzma.is_some() {
        let mut name = raw_part.as_os_str().to_owned();
        name.push(".lzma");
        PathBuf::from(name)
    } else {
        raw_part.clone()
    };
    let url = job.source();
    let mut offset = 0;
    let mut request = super::agent()
        .get(url)
        .header("Accept-Encoding", "identity");
    if job.has_hash() || validator.is_some() {
        let len = partial_len(&tmp)?;
        if len > 0 && job.transfer_size().is_none_or(|size| len < size) {
            offset = len;
            request = request.header("Range", format!("bytes={offset}-"));
            if let Some(validator) = validator.as_ref() {
                request = request.header("If-Range", &validator.etag);
            }
        }
    }
    let response = request
        .config()
        .timeout_global(timeout)
        .timeout_connect(connect_timeout(url))
        .timeout_recv_body(Some(body_timeout(job.transfer_size())))
        .build()
        .call();
    if offset > 0 && matches!(response, Err(ureq::Error::StatusCode(416))) {
        fs::remove_file(&tmp).at(&tmp)?;
        *validator = None;
    }
    let mut resp = response.map_err(transfer_error)?;
    let identity = identity_body(&resp);
    let mut size = job.transfer_size();
    let mut response_end = None;
    match resp.status().as_u16() {
        200 => {
            offset = 0;
            // Content-Length is meaningful for bytes on disk only with identity encoding.
            if identity && size.is_none() {
                size = resp.body().content_length();
            }
        }
        206 => {
            let range = content_range(&resp).and_then(|(start, end, total)| {
                let total = total?;
                (offset > 0
                    && identity
                    && start == offset
                    && size.is_none_or(|size| size == total)
                    && resp
                        .body()
                        .content_length()
                        .is_none_or(|len| len == end + 1 - offset)
                    && validator
                        .as_ref()
                        .is_none_or(|validator| validator.matches(&resp)))
                .then_some((end + 1, total))
            });
            let Some((end, total)) = range else {
                if offset > 0 {
                    fs::remove_file(&tmp).at(&tmp)?;
                    *validator = None;
                }
                return Err(Error::Http(
                    "response has an unexpected Content-Range".into(),
                ));
            };
            response_end = Some(end);
            size = Some(total);
        }
        status => return Err(request_error(ureq::Error::StatusCode(status))),
    }
    let hashed = job.has_hash();
    if offset == 0 {
        *validator = if identity && !hashed {
            Validator::from_response(&resp)
        } else {
            None
        };
    }
    let preserve = identity && (hashed || validator.is_some());

    let on_bytes = |n: u64| {
        transferred.set(transferred.get() + n);
        reporter.add_bytes(n);
    };
    let tail = TailReader {
        body: resp.body_mut().as_reader(),
        reporter,
        retry_slow_tail,
        window: None,
        received: offset,
        size,
    };
    let file = fs::OpenOptions::new()
        .read(offset > 0)
        .write(true)
        .create(true)
        .truncate(offset == 0)
        .open(&tmp)
        .at(&tmp)?;
    let mut output =
        Hashing::new(file, &on_bytes).with_md5(job.lzma.is_none() && job.md5.is_some());
    if offset > 0 {
        output.hash_prefix(offset).at(&tmp)?;
    }
    let limit = response_end.or(size).unwrap_or(u64::MAX);
    let mut tail = tail.take((limit - offset).saturating_add(1));
    let result = copy(&mut tail, &mut output, &tmp)
        .and_then(|()| output.flush().at(&tmp))
        .and_then(|()| {
            if output.count > limit {
                return Err(Error::Checksum(job.file_name().into_owned()));
            }
            if size.is_some_and(|size| output.count < size) {
                return Err(Error::DownloadInterrupted(
                    "response ended before the file was complete".into(),
                ));
            }
            let valid = size.is_none_or(|size| size == output.count)
                && match &job.lzma {
                    Some(compressed) => compressed.sha1.eq_ignore_ascii_case(&output.hex_digest()),
                    None => raw_matches(job, &output),
                };
            if !valid {
                return Err(Error::Checksum(job.file_name().into_owned()));
            }
            Ok(())
        });
    drop(output);
    if let Err(e) = result {
        if !preserve || !matches!(e, Error::Http(_) | Error::DownloadInterrupted(_)) {
            fs::remove_file(&tmp).at(&tmp)?;
            *validator = None;
        }
        return Err(e);
    }
    if job.lzma.is_some() {
        let result = decompress(job, &tmp, &raw_part);
        fs::remove_file(&tmp).at(&tmp)?;
        if let Err(e) = result {
            let _ = fs::remove_file(&raw_part);
            return Err(e);
        }
    }
    fs::rename(&raw_part, &job.dest).at(&job.dest)
}

fn decompress(job: &DownloadJob, compressed: &Path, raw: &Path) -> Result<()> {
    let mut input = BufReader::with_capacity(BUF_SIZE, fs::File::open(compressed).at(compressed)?);
    let mut output =
        Hashing::new(fs::File::create(raw).at(raw)?, &|_| {}).with_md5(job.md5.is_some());
    lzma_rs::lzma_decompress(&mut input, &mut output).map_err(|e| {
        Error::Other(format!(
            "corrupt compressed download of {} from {}: {e}",
            job.file_name(),
            host(job.source())
        ))
    })?;
    output.flush().at(raw)?;
    if !raw_matches(job, &output) {
        return Err(Error::Checksum(job.file_name().into_owned()));
    }
    Ok(())
}

/// Single bounded attempt for background prefetch. Stage beside the destination with
/// its own temporary name so a concurrent launch can use the regular download path.
pub(crate) fn download_before(job: DownloadJob, deadline: Instant) -> Result<()> {
    if job.is_satisfied() {
        return Ok(());
    }
    let dest = job.dest.clone();
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).at(parent)?;
    let stage = tempfile::tempdir_in(parent).at(parent)?;
    let job = DownloadJob {
        dest: stage.path().join(dest.file_name().unwrap_or_default()),
        ..job
    };
    let reporter = Reporter {
        stage: "Prefetching loader libraries",
        progress: &|_| {},
        start: Instant::now(),
        last_ms: AtomicU64::new(0),
        files_done: AtomicU64::new(0),
        files_total: 1,
        bytes_done: AtomicU64::new(0),
        bytes_total: job.transfer_size().unwrap_or(0),
        all_sizes_known: job.transfer_size().is_some(),
        fast_batch: Default::default(),
    };
    download_one(
        &job,
        &reporter,
        &Cell::new(0),
        false,
        Some(super::time_remaining(deadline)?),
        &mut None,
    )
    .map_err(|e| download_error(&job, e))?;
    if !job.is_satisfied() {
        return Err(Error::Other(format!(
            "invalid library: {}",
            job.file_name()
        )));
    }
    fs::rename(&job.dest, &dest).at(&dest)
}

/// Detect a crawling final file on a batch that has already demonstrated fast throughput.
struct TailReader<'a, 'b, R> {
    body: R,
    reporter: &'a Reporter<'b>,
    retry_slow_tail: bool,
    window: Option<(Instant, u64)>,
    received: u64,
    size: Option<u64>,
}

impl<R: Read> Read for TailReader<'_, '_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.retry_slow_tail && self.window.is_none() && self.reporter.fast_tail() {
            self.window = Some((Instant::now(), self.received));
        }
        let n = with_download_idle(|| self.body.read(buf))?;
        self.received += n as u64;
        if n > 0
            && let Some((start, received)) = self.window
        {
            let elapsed = start.elapsed();
            if elapsed >= CHUNK_STALL {
                let remaining = self
                    .size
                    .map_or(u64::MAX, |size| size.saturating_sub(self.received));
                if crawling(elapsed, self.received - received, remaining) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "final download is crawling; retrying",
                    ));
                }
                self.window = Some((Instant::now(), self.received));
            }
        }
        Ok(n)
    }
}

fn fast_transfer(bytes: u64, start: Instant) -> bool {
    bytes >= FAST_BATCH_SPEED
        && bytes as f64 >= FAST_BATCH_SPEED as f64 * start.elapsed().as_secs_f64()
}

fn crawling(elapsed: Duration, received: u64, remaining: u64) -> bool {
    let rate = received as f64 / elapsed.as_secs_f64();
    rate < TAIL_MIN_SPEED as f64 && remaining as f64 > rate * 2.0
}

/// Byte ranges `[start, end)` for a large, uncompressed file of known size: chunks of
/// [`CHUNK_SIZE`], fetched by [`CONNECTIONS`] connections taking turns.
fn segments(job: &DownloadJob) -> Option<Vec<(u64, u64)>> {
    let size = job.size.filter(|s| *s >= SEGMENT_MIN_SIZE)?;
    if job.lzma.is_some() || job.md5.is_some() {
        return None;
    }
    Some(
        (0..size.div_ceil(CHUNK_SIZE))
            .map(|i| (i * CHUNK_SIZE, ((i + 1) * CHUNK_SIZE).min(size)))
            .collect(),
    )
}

enum Segmented {
    /// The server ignored the Range header; download the usual way.
    NoRanges,
    /// The batch reduced concurrency while this file was segmented.
    UseStream,
    Failed(Error),
}

impl From<Error> for Segmented {
    fn from(e: Error) -> Self {
        Segmented::Failed(e)
    }
}

/// One chunk of a big file.
struct Chunk {
    start: u64,
    end: u64,
    state: Mutex<ChunkState>,
}

#[derive(Default)]
struct ChunkState {
    /// Bytes reported by the primary attempt.
    got: u64,
    done: bool,
    committed: bool,
    hedged: bool,
    sample: Option<(Instant, u64)>,
    moved: Option<Instant>,
}

impl ChunkState {
    fn should_hedge(&mut self, len: u64, now: Instant, fast: bool) -> bool {
        if self.done || self.hedged {
            return false;
        }
        let Some((start, received)) = self.sample else {
            return false;
        };
        let elapsed = now.duration_since(start);
        if self
            .moved
            .is_some_and(|at| now.duration_since(at) >= CHUNK_STALL)
        {
            return true;
        }
        if elapsed < CHUNK_STALL {
            return false;
        }
        self.sample = Some((now, self.got));
        fast && crawling(elapsed, self.got.saturating_sub(received), len - self.got)
    }

    fn record(&mut self, n: u64, tx: &std::sync::mpsc::Sender<ChunkNews>) {
        if !self.done {
            self.got += n;
            self.moved = Some(Instant::now());
            let _ = tx.send(ChunkNews::Bytes(n));
        }
    }

    fn complete(&mut self, len: u64, tx: &std::sync::mpsc::Sender<ChunkNews>) {
        self.done = true;
        self.committed = true;
        let _ = tx.send(ChunkNews::Bytes(len - self.got));
        let _ = tx.send(ChunkNews::Done);
    }
}

/// What the chunk fetchers tell the download.
enum ChunkNews {
    Bytes(u64),
    /// A fetch failed partway and is starting over: its bytes no longer count.
    Unreport(u64),
    Done,
    NoRanges,
    UseStream,
    Failed(Error),
}

struct Chunks {
    job: DownloadJob,
    tmp: PathBuf,
    list: Vec<Chunk>,
    next: std::sync::atomic::AtomicUsize,
    limits: Arc<TransferLimits>,
}

struct SegmentedPart {
    remaining: Vec<(u64, u64)>,
    initialized: bool,
}

impl SegmentedPart {
    fn new(job: &DownloadJob) -> Option<Self> {
        Some(Self {
            remaining: segments(job)?,
            initialized: false,
        })
    }

    fn discard(&mut self, job: &DownloadJob, tmp: &Path) -> Result<()> {
        fs::remove_file(tmp).at(tmp)?;
        *self = Self::new(job).expect("discarding a segmented file");
        Ok(())
    }
}

/// Fetch a big file in chunks into one preallocated `.part` file, then hash the whole file and
/// rename it into place. [`CONNECTIONS`] connections take chunks in turn, each with a timeout
/// sized for the chunk (a stalled connection costs one chunk, not the file), and a chunk that
/// stalls or crawls after the rest are handed out is fetched a second time.
/// Whichever copy finishes first commits the range.
fn download_segmented(
    job: &DownloadJob,
    reporter: &Reporter,
    limits: &Arc<TransferLimits>,
    transferred: &Cell<u64>,
    part: &mut SegmentedPart,
) -> std::result::Result<(), Segmented> {
    if let Some(parent) = job.dest.parent() {
        fs::create_dir_all(parent).at(parent)?;
    }
    let tmp = part_path(&job.dest);
    let size = job.size.expect("segmented downloads have a known size");
    if !part.initialized {
        let file = fs::File::create(&tmp).at(&tmp)?;
        file.set_len(size).at(&tmp)?;
        part.initialized = true;
    }
    let retained = size
        - part
            .remaining
            .iter()
            .map(|(start, end)| end - start)
            .sum::<u64>();
    transferred.set(retained);
    reporter.add_bytes(retained);
    let chunks = Arc::new(Chunks {
        job: job.clone(),
        tmp: tmp.clone(),
        list: part
            .remaining
            .iter()
            .map(|&(start, end)| Chunk {
                start,
                end,
                state: Mutex::default(),
            })
            .collect(),
        next: Default::default(),
        limits: limits.clone(),
    });
    let (tx, rx) = std::sync::mpsc::channel::<ChunkNews>();
    let mut workers: Vec<_> = (0..connections().min(chunks.list.len()))
        .map(|_| {
            let (chunks, tx) = (chunks.clone(), tx.clone());
            std::thread::spawn(move || {
                loop {
                    let i = chunks.next.fetch_add(1, Ordering::Relaxed);
                    if i >= chunks.list.len() || !fetch_chunk(&chunks, i, &tx) {
                        return;
                    }
                }
            })
        })
        .collect();
    let start = Instant::now();
    let mut done = 0;
    let mut fast_file = false;
    let mut next_check = start + CHUNK_STALL / 4;
    let result = loop {
        if done == chunks.list.len() {
            break Ok(());
        }
        if !limits.allows_ranges() {
            break Err(Segmented::UseStream);
        }
        let now = Instant::now();
        if now >= next_check {
            workers.extend(hedge_stalled(
                &chunks,
                &tx,
                fast_file || reporter.fast_tail(),
            ));
            next_check = now + CHUNK_STALL / 4;
        }
        match rx.recv_timeout(next_check.saturating_duration_since(Instant::now())) {
            Ok(ChunkNews::Bytes(n)) => {
                transferred.set(transferred.get() + n);
                reporter.add_bytes(n);
            }
            Ok(ChunkNews::Unreport(n)) => {
                transferred.set(transferred.get().saturating_sub(n));
                reporter.remove_bytes(n);
            }
            Ok(ChunkNews::Done) => {
                done += 1;
                fast_file |= fast_transfer(transferred.get() - retained, start);
            }
            Ok(ChunkNews::NoRanges) => break Err(Segmented::NoRanges),
            Ok(ChunkNews::UseStream) => break Err(Segmented::UseStream),
            Ok(ChunkNews::Failed(e)) => break Err(Segmented::Failed(e)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break Err(Segmented::Failed(Error::Http(
                    "range download stopped".into(),
                )));
            }
        }
    };
    // A worker may still be reading. Once all chunks are closed under their
    // locks, only committed ranges survive and no worker can write again.
    let mut chunk = chunks.list.iter();
    part.remaining.retain(|_| {
        let mut state = chunk.next().unwrap().state.lock().unwrap();
        state.done = true;
        !state.committed
    });
    // Keep the file's admission slot until all range requests have drained.
    // This also prevents a fallback stream from overlapping the old requests.
    for worker in workers {
        worker.join().expect("range worker panicked");
    }
    if let Err(e) = result {
        if matches!(e, Segmented::NoRanges) || job.sha1.is_none() {
            part.discard(job, &tmp)?;
        }
        return Err(e);
    }
    let size_ok = fs::metadata(&tmp).at(&tmp)?.len() == size;
    let hash_ok = match &job.sha1 {
        Some(want) => want.eq_ignore_ascii_case(&sha1_file(&tmp)?),
        None => true,
    };
    if !size_ok || !hash_ok {
        part.discard(job, &tmp)?;
        return Err(Error::Checksum(job.file_name().into_owned()).into());
    }
    fs::rename(&tmp, &job.dest).at(&job.dest)?;
    Ok(())
}

/// Once every chunk is handed out, allow one extra request across the batch.
fn hedge_stalled(
    chunks: &Arc<Chunks>,
    tx: &std::sync::mpsc::Sender<ChunkNews>,
    fast: bool,
) -> Option<std::thread::JoinHandle<()>> {
    if chunks.next.load(Ordering::Relaxed) < chunks.list.len() {
        return None;
    }
    let hedge = chunks.limits.try_hedge()?;
    let now = Instant::now();
    for (i, chunk) in chunks.list.iter().enumerate() {
        let mut state = chunk.state.lock().unwrap();
        if state.should_hedge(chunk.end - chunk.start, now, fast) {
            state.hedged = true;
            log::info!(
                "chunk {i} of {} from {} is slow: fetching it again",
                chunks.job.file_name(),
                host(chunks.job.source())
            );
            let (chunks, tx) = (chunks.clone(), tx.clone());
            return Some(std::thread::spawn(move || {
                let _hedge = hedge;
                if let Err(Segmented::Failed(e)) = fetch_once(&chunks, i, &tx, false) {
                    let job = &chunks.job;
                    let e = download_error(job, e);
                    log::warn!(
                        "hedged chunk {i} of {} from {} failed: {e}",
                        job.file_name(),
                        host(job.source())
                    );
                }
            }));
        }
    }
    None
}

/// Fetch chunk `i`, retrying a few times. False: the download is over (failed, or no ranges).
fn fetch_chunk(chunks: &Chunks, i: usize, tx: &std::sync::mpsc::Sender<ChunkNews>) -> bool {
    let mut attempt = 1;
    loop {
        if !chunks.limits.allows_ranges() {
            let _ = tx.send(ChunkNews::UseStream);
            return false;
        }
        match fetch_once(chunks, i, tx, true) {
            Ok(()) => return true,
            Err(Segmented::NoRanges) => {
                let mut state = chunks.list[i].state.lock().unwrap();
                if state.done {
                    return true;
                }
                state.done = true;
                let _ = tx.send(ChunkNews::NoRanges);
                return false;
            }
            Err(Segmented::Failed(e)) => {
                let chunk = &chunks.list[i];
                // The second fetch won meanwhile: it counted the rest of the bytes.
                {
                    let mut state = chunk.state.lock().unwrap();
                    if state.done {
                        return true;
                    }
                    let reported = std::mem::take(&mut state.got);
                    state.sample = None;
                    state.moved = None;
                    let _ = tx.send(ChunkNews::Unreport(reported));
                    let job = &chunks.job;
                    log::warn!(
                        "chunk {i} attempt {attempt} for {} from {} failed: {e}",
                        job.file_name(),
                        host(job.source())
                    );
                    if matches!(e, Error::DownloadInterrupted(_)) {
                        chunks.limits.interrupted();
                        if !chunks.limits.allows_ranges() {
                            let _ = tx.send(ChunkNews::UseStream);
                            return false;
                        }
                    }
                    if attempt >= ATTEMPTS || is_not_found(&e) {
                        state.done = true;
                        let _ = tx.send(ChunkNews::Failed(e));
                        return false;
                    }
                }
                std::thread::sleep(RETRY_BACKOFF * attempt);
                attempt += 1;
            }
            Err(Segmented::UseStream) => unreachable!("only the coordinator switches streams"),
        }
    }
}

/// One fetch of chunk `i` into its place in the `.part` file. The chunk's own fetch
/// (`own`) reports its bytes as they come; a second one reports what's left when it wins.
fn fetch_once(
    chunks: &Chunks,
    i: usize,
    tx: &std::sync::mpsc::Sender<ChunkNews>,
    own: bool,
) -> std::result::Result<(), Segmented> {
    use std::io::{Seek, SeekFrom};
    let (job, chunk) = (&chunks.job, &chunks.list[i]);
    let len = chunk.end - chunk.start;
    {
        let mut state = chunk.state.lock().unwrap();
        if state.done {
            return Ok(());
        }
        if own {
            let now = Instant::now();
            state.sample = Some((now, 0));
            state.moved = Some(now);
        }
    }
    let mut resp = super::agent()
        .get(&job.url)
        .header("Range", &format!("bytes={}-{}", chunk.start, chunk.end - 1))
        .header("Accept-Encoding", "identity")
        .config()
        .timeout_connect(connect_timeout(&job.url))
        .timeout_recv_body(Some(chunk_timeout(len)))
        .build()
        .call()
        .map_err(transfer_error)?;
    if resp.status().as_u16() != 206 {
        return Err(Segmented::NoRanges);
    }
    if !identity_body(&resp)
        || !content_range(&resp).is_some_and(|(start, end, total)| {
            start == chunk.start
                && end == chunk.end - 1
                && total.is_none_or(|n| Some(n) == job.size)
        })
    {
        log::warn!(
            "range request for {} from {} returned an unexpected Content-Range",
            job.file_name(),
            host(job.source())
        );
        return Err(Segmented::NoRanges);
    }
    let tmp = &chunks.tmp;
    let mut body = resp.body_mut().as_reader();
    let mut bytes = vec![0; len as usize];
    let mut got = 0;
    while got < bytes.len() {
        let n = with_download_idle(|| body.read(&mut bytes[got..]))
            .map_err(|e| transfer_error(e.into()))?;
        if n == 0 {
            break;
        }
        let mut state = chunk.state.lock().unwrap();
        if state.done {
            return Ok(());
        }
        got += n;
        if own {
            state.record(n as u64, tx);
        }
    }
    if got != bytes.len() {
        return Err(Error::DownloadInterrupted(
            "response ended before the range was complete".into(),
        )
        .into());
    }
    let mut state = chunk.state.lock().unwrap();
    if !state.done {
        // Close before publishing completion, including on Windows.
        {
            let mut out = fs::OpenOptions::new().write(true).open(tmp).at(tmp)?;
            out.seek(SeekFrom::Start(chunk.start)).at(tmp)?;
            out.write_all(&bytes).at(tmp)?;
        }
        state.complete(len, tx);
    }
    Ok(())
}

/// A chunk's body timeout: room for a slow line, short enough that a dead connection is noticed.
fn chunk_timeout(len: u64) -> Duration {
    BODY_TIMEOUT_BASE + Duration::from_secs(len / CHUNK_MIN_SPEED)
}

/// Match the expected size and available checksums of the final file.
fn raw_matches<T>(job: &DownloadJob, output: &Hashing<T>) -> bool {
    job.size.is_none_or(|s| s == output.count)
        && job
            .sha1
            .as_deref()
            .is_none_or(|want| want.eq_ignore_ascii_case(&output.hex_digest()))
        && job.md5.as_deref().is_none_or(|want| {
            output
                .md5_digest()
                .is_some_and(|got| want.eq_ignore_ascii_case(&got))
        })
}

fn copy(input: &mut impl Read, output: &mut impl Write, path: &Path) -> Result<()> {
    let mut buf = vec![0u8; BUF_SIZE];
    loop {
        let n = input.read(&mut buf).map_err(|e| transfer_error(e.into()))?;
        if n == 0 {
            return Ok(());
        }
        output.write_all(&buf[..n]).at(path)?;
    }
}

/// Reader/writer adapter that SHA-1s and counts everything passing through.
struct Hashing<'a, T> {
    inner: T,
    hasher: Sha1,
    md5: Option<Md5>,
    count: u64,
    on_bytes: &'a dyn Fn(u64),
}

impl<'a, T> Hashing<'a, T> {
    fn new(inner: T, on_bytes: &'a dyn Fn(u64)) -> Self {
        Self {
            inner,
            hasher: Sha1::new(),
            md5: None,
            count: 0,
            on_bytes,
        }
    }

    fn hex_digest(&self) -> String {
        hex::encode(self.hasher.clone().finalize())
    }

    /// Also compute the MD5 (for jobs that carry one).
    fn with_md5(mut self, on: bool) -> Self {
        self.md5 = on.then(Md5::new);
        self
    }

    fn md5_digest(&self) -> Option<String> {
        self.md5.as_ref().map(|m| hex::encode(m.clone().finalize()))
    }

    fn record(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
        if let Some(m) = &mut self.md5 {
            m.update(bytes);
        }
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

impl<T: Read> Hashing<'_, T> {
    fn hash_prefix(&mut self, mut remaining: u64) -> std::io::Result<()> {
        let mut buf = vec![0; BUF_SIZE];
        while remaining > 0 {
            let len = remaining.min(BUF_SIZE as u64) as usize;
            self.read_exact(&mut buf[..len])?;
            remaining -= len as u64;
        }
        Ok(())
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
pub(crate) fn part_path(dest: &Path) -> PathBuf {
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
mod resilience_tests;

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
            md5: None,
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
    fn cached_jars_can_have_zip_comments_but_need_readable_local_headers() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("library.JAR");
        let mut jar = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        jar.set_comment("a ZIP comment").unwrap();
        jar.start_file("test.class", zip::write::SimpleFileOptions::default())
            .unwrap();
        jar.write_all(b"class data").unwrap();
        let mut bytes = jar.finish().unwrap().into_inner();
        let entry = DownloadJob {
            dest: dest.clone(),
            ..job("", Some(bytes.len() as u64))
        };
        fs::write(&dest, &bytes).unwrap();
        assert!(entry.is_satisfied());
        bytes[0] ^= 1;
        fs::write(&dest, &bytes).unwrap();
        assert!(!entry.is_satisfied());
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
        let pending = pending_jobs(
            vec![
                job("missing/a", Some(1)),
                job("missing/a", Some(1)),
                job("missing/b", Some(300)),
                job("missing/c", None),
            ],
            Vec::new(),
        );
        let order: Vec<_> = pending.iter().map(|j| j.dest.clone()).collect();
        assert_eq!(
            order,
            ["missing/b", "missing/a", "missing/c"].map(PathBuf::from)
        );
    }

    #[test]
    fn parallel_checks_keep_missing_and_truncated_files_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut jobs = Vec::new();
        for i in 0..256 {
            let dest = dir.path().join(i.to_string());
            if i % 3 != 0 {
                fs::write(&dest, if i % 3 == 1 { &b"ok"[..] } else { &b"x"[..] }).unwrap();
            }
            let entry = DownloadJob {
                dest,
                ..job("", Some(2))
            };
            jobs.extend([entry.clone(), entry]);
        }
        let pending = pending_jobs(jobs, Vec::new());
        assert_eq!(pending.len(), 171);
        let unique: HashSet<_> = pending.iter().map(|j| j.dest.clone()).collect();
        assert_eq!(unique.len(), pending.len());
        assert!(pending.iter().all(|j| !j.is_satisfied()));
    }

    #[test]
    fn hashing_adapter_counts_and_hashes() {
        let seen = Cell::new(0);
        let on = |n: u64| seen.set(seen.get() + n);
        let mut r = Hashing::new(std::io::Cursor::new(b"abc".to_vec()), &on);
        let mut sink = Hashing::new(Vec::new(), &|_| {});
        copy(&mut r, &mut sink, Path::new("sink")).unwrap();
        assert_eq!((r.count, sink.count, seen.get()), (3, 3, 3));
        assert_eq!(
            sink.hex_digest(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
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
    fn large_files_split_into_chunks_that_cover_them() {
        assert!(segments(&job("a", Some(1024))).is_none());
        assert!(segments(&job("a", None)).is_none());
        let size = 20 * 1024 * 1024 + 3;
        let ranges = segments(&job("a", Some(size))).unwrap();
        assert_eq!(ranges.len(), 11);
        assert_eq!(ranges[0].0, 0);
        assert_eq!(ranges.last().unwrap().1, size);
        assert!(ranges.windows(2).all(|w| w[0].1 == w[1].0));
        assert!(ranges.iter().all(|(a, b)| b - a <= CHUNK_SIZE));
    }

    /// A local server that stalls or trickles on the first request for offset zero.
    fn stalling_server(data: std::sync::Arc<Vec<u8>>, trickle: bool) -> String {
        use std::io::{BufRead, BufReader as Lines};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stalled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (data, stalled) = (data.clone(), stalled.clone());
                std::thread::spawn(move || {
                    let mut reader = Lines::new(stream.try_clone().unwrap());
                    let mut range = None;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                            break;
                        }
                        if let Some(r) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                            let (a, b) = r.trim().split_once('-').unwrap();
                            range =
                                Some((a.parse::<usize>().unwrap(), b.parse::<usize>().unwrap()));
                        }
                    }
                    let Some((start, end)) = range else { return };
                    let slow = start == 0 && !stalled.swap(true, Ordering::Relaxed);
                    let body = &data[start..=end];
                    let mut out = stream;
                    let head = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nConnection: close\r\n\r\n",
                        body.len(),
                        data.len()
                    );
                    if out.write_all(head.as_bytes()).is_err() {
                        return;
                    }
                    if slow && trickle {
                        // Keep sending at about 40 KiB/s. A separate copy must win even
                        // though the primary keeps generating progress notifications.
                        for chunk in body.chunks(4096) {
                            if out.write_all(chunk).is_err() {
                                return;
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    } else if slow {
                        let _ = reader
                            .get_mut()
                            .set_read_timeout(Some(Duration::from_secs(20)));
                        let _ = reader.get_mut().read(&mut [0]);
                    } else {
                        let _ = out.write_all(body);
                    }
                });
            }
        });
        format!("http://{addr}/big.jar")
    }

    #[test]
    fn a_stalled_connection_is_fetched_again_instead_of_waited_for() {
        assert_tail_range_is_hedged(false);
    }

    #[test]
    fn a_trickling_range_is_hedged_despite_continuous_progress() {
        assert_tail_range_is_hedged(true);
    }

    fn assert_tail_range_is_hedged(trickle: bool) {
        let size = SEGMENT_MIN_SIZE as usize + 12345;
        let data: Vec<u8> = (0..size).map(|i| (i * 31 % 251) as u8).collect();
        let sha1 = hex::encode(Sha1::digest(&data));
        let url = stalling_server(std::sync::Arc::new(data.clone()), trickle);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("big.jar");
        let started = Instant::now();
        download_all(
            "x",
            vec![DownloadJob {
                url,
                dest: dest.clone(),
                sha1: Some(sha1),
                size: Some(size as u64),
                lzma: None,
                md5: None,
            }],
            &|_| {},
        )
        .unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "took {:?}",
            started.elapsed()
        );
        assert_eq!(fs::read(&dest).unwrap(), data);
    }

    #[test]
    fn primary_bytes_arriving_after_hedge_completion_are_ignored() {
        let mut state = ChunkState::default();
        let (tx, rx) = std::sync::mpsc::channel();
        state.record(100, &tx);
        state.complete(200, &tx);
        state.record(100, &tx);
        assert!(matches!(rx.try_recv(), Ok(ChunkNews::Bytes(100))));
        assert!(matches!(rx.try_recv(), Ok(ChunkNews::Bytes(100))));
        assert!(matches!(rx.try_recv(), Ok(ChunkNews::Done)));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn progress_stays_bounded_with_files_left() {
        for all_sizes_known in [true, false] {
            let progress = |p: ProgressInfo<'_>| {
                assert_eq!(p.done, 7);
                assert_eq!(p.total, 8);
                assert_eq!(p.bytes_total, if all_sizes_known { 1000 } else { 0 });
                assert_eq!(p.bytes_done, p.bytes_total);
            };
            let reporter = Reporter {
                stage: "test",
                progress: &progress,
                start: Instant::now(),
                last_ms: AtomicU64::new(0),
                files_done: AtomicU64::new(7),
                files_total: 8,
                bytes_done: AtomicU64::new(1100),
                bytes_total: 1000,
                all_sizes_known,
                fast_batch: Default::default(),
            };
            reporter.report(true);
        }
    }

    #[test]
    fn a_slow_link_does_not_hedge_an_active_range() {
        let now = Instant::now();
        let mut state = ChunkState {
            got: 40 * 1024,
            sample: Some((now - CHUNK_STALL, 0)),
            moved: Some(now),
            ..Default::default()
        };
        assert!(!state.should_hedge(CHUNK_SIZE, now, false));
        assert!(state.should_hedge(CHUNK_SIZE, now + CHUNK_STALL, false));
    }

    #[test]
    fn a_trickling_final_file_is_retried() {
        use std::io::BufRead;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = std::sync::Arc::new(AtomicU64::new(0));
        let server_hits = hits.clone();
        let fast = 4 * 1024 * 1024;
        let tail = 512 * 1024;
        let server = std::thread::spawn(move || {
            std::thread::scope(|scope| {
                listener.set_nonblocking(true).unwrap();
                let deadline = Instant::now() + Duration::from_secs(20);
                let mut accepted = 0;
                while accepted < 3 && Instant::now() < deadline {
                    let (mut stream, _) = match listener.accept() {
                        Ok(conn) => conn,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        Err(e) => panic!("{e}"),
                    };
                    accepted += 1;
                    stream.set_nonblocking(false).unwrap();
                    let server_hits = &server_hits;
                    scope.spawn(move || {
                        let mut headers = BufReader::new(&mut stream);
                        let mut line = String::new();
                        headers.read_line(&mut line).unwrap();
                        let is_tail = line.starts_with("GET /tail.jar ");
                        while !line.is_empty() && line != "\r\n" {
                            line.clear();
                            headers.read_line(&mut line).unwrap();
                        }
                        let size = if is_tail { tail } else { fast };
                        let slow = is_tail && server_hits.fetch_add(1, Ordering::Relaxed) == 0;
                        write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n"
                        )
                        .unwrap();
                        if slow {
                            for _ in 0..size / 4096 {
                                if stream.write_all(&[0; 4096]).is_err() {
                                    return;
                                }
                                std::thread::sleep(Duration::from_millis(100));
                            }
                        } else {
                            stream.write_all(&vec![0; size]).unwrap();
                        }
                    });
                }
            });
        });
        let dir = tempfile::tempdir().unwrap();
        let jobs = [("fast.jar", fast), ("tail.jar", tail)].map(|(name, size)| DownloadJob {
            url: format!("http://{addr}/{name}"),
            dest: dir.path().join(name),
            size: Some(size as u64),
            sha1: Some(sha1_bytes(&vec![0; size])),
            ..job("", None)
        });
        download_all("test", Vec::from(jobs), &|p| {
            assert!(p.bytes_done <= p.bytes_total);
            if p.done == 2 {
                assert_eq!(p.bytes_done, (fast + tail) as u64);
            }
        })
        .unwrap();
        assert_eq!(hits.load(Ordering::Relaxed), 2);
        server.join().unwrap();
    }

    #[test]
    fn a_stalled_chunk_gives_up_in_minutes_not_an_hour() {
        // The old ranges were up to 40 MB with an 8 KB/s bar: an 85-minute wait.
        assert!(chunk_timeout(CHUNK_SIZE) <= Duration::from_secs(4 * 60));
    }

    #[test]
    fn body_timeout_scales_with_size() {
        assert_eq!(body_timeout(None), BODY_TIMEOUT_UNKNOWN_SIZE);
        let big = body_timeout(Some(100 * 1024 * 1024));
        assert_eq!(big, BODY_TIMEOUT_BASE + Duration::from_secs(100 * 128));
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
    /// Local HTTP server: answers each request with the next status from `script`
    /// (`200` serves `body`, repeating the last entry); returns its base URL and hit counter.
    fn serve(script: Vec<u16>, body: &'static [u8]) -> (String, std::sync::Arc<AtomicU64>) {
        use std::io::Write as _;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = std::sync::Arc::new(AtomicU64::new(0));
        let counter = hits.clone();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let n = counter.fetch_add(1, Ordering::SeqCst) as usize;
                let status = script[n.min(script.len() - 1)];
                let mut buf = [0u8; 4096];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                let body = if status == 200 { body } else { b"" };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(body);
            }
        });
        (base, hits)
    }

    fn sha1_bytes(b: &[u8]) -> String {
        Sha1::digest(b).iter().map(|x| format!("{x:02x}")).collect()
    }

    fn fetch(url: &str, mirror: Option<String>, sha1: Option<&str>) -> (Result<()>, PathBuf) {
        let dir = tempfile::tempdir().unwrap().keep();
        let job = DownloadJob {
            url: url.to_string(),
            dest: dir.join("f.jar"),
            sha1: sha1.map(String::from),
            ..job("", None)
        };
        let progress = |_: crate::ProgressInfo<'_>| {};
        let reporter = Reporter {
            stage: "t",
            progress: &progress,
            start: Instant::now(),
            last_ms: AtomicU64::new(0),
            files_done: AtomicU64::new(0),
            files_total: 1,
            bytes_done: AtomicU64::new(0),
            bytes_total: 0,
            all_sizes_known: false,
            fast_batch: Default::default(),
        };
        let policy = Policy {
            attempts: 3,
            delays: vec![Duration::from_millis(5)],
        };
        let dest = job.dest.clone();
        (
            download_with_alternates(
                &job,
                &reporter,
                &Arc::new(TransferLimits::new(1)),
                mirror,
                &policy,
            ),
            dest,
        )
    }

    #[test]
    fn retries_server_errors_then_succeeds() {
        let (base, hits) = serve(vec![503, 502, 200], b"jar");
        let (res, dest) = fetch(&format!("{base}/a.jar"), None, None);
        res.unwrap();
        assert_eq!(fs::read(dest).unwrap(), b"jar");
        assert_eq!(hits.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn not_found_is_not_retried_or_mirrored() {
        let (base, hits) = serve(vec![404], b"");
        let (mirror, mirror_hits) = serve(vec![200], b"jar");
        let (res, _) = fetch(
            &format!("{base}/a.jar"),
            Some(format!("{mirror}/a.jar")),
            None,
        );
        assert!(is_not_found(&res.unwrap_err()));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(mirror_hits.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn not_found_falls_back_to_mirror_when_hash_matches() {
        let (base, hits) = serve(vec![404], b"");
        let (mirror, mirror_hits) = serve(vec![200], b"jar");
        let sha = sha1_bytes(b"jar");
        let (res, dest) = fetch(
            &format!("{base}/a.jar"),
            Some(format!("{mirror}/a.jar")),
            Some(&sha),
        );
        res.unwrap();
        assert_eq!(fs::read(dest).unwrap(), b"jar");
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(mirror_hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn not_found_with_bad_mirror_hash_keeps_not_found_error() {
        let (base, _) = serve(vec![404], b"");
        let (mirror, _) = serve(vec![200], b"jar");
        let bad = "0".repeat(40);
        let (res, dest) = fetch(
            &format!("{base}/a.jar"),
            Some(format!("{mirror}/a.jar")),
            Some(&bad),
        );
        assert!(is_not_found(&res.unwrap_err()));
        assert!(!dest.exists());
    }

    #[test]
    fn not_found_installer_is_mirrored_only_with_a_hash() {
        let (base, _) = serve(vec![404], b"");
        let (mirror, mirror_hits) = serve(vec![200], b"jar");
        let sha = sha1_bytes(b"jar");
        let url = format!("{base}/forge-1-installer.jar");
        let mirror_url = format!("{mirror}/forge-1-installer.jar");
        let (res, _) = fetch(&url, Some(mirror_url.clone()), None);
        assert!(is_not_found(&res.unwrap_err()));
        assert_eq!(mirror_hits.load(Ordering::SeqCst), 0);
        let (res, dest) = fetch(&url, Some(mirror_url), Some(&sha));
        res.unwrap();
        assert_eq!(fs::read(dest).unwrap(), b"jar");
        assert_eq!(mirror_hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn mirror_used_after_primary_keeps_failing() {
        let (base, hits) = serve(vec![500], b"");
        let (mirror, _) = serve(vec![200], b"jar");
        let sha = sha1_bytes(b"jar");
        let (res, dest) = fetch(
            &format!("{base}/a.jar"),
            Some(format!("{mirror}/a.jar")),
            Some(&sha),
        );
        res.unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 3);
        assert_eq!(fs::read(dest).unwrap(), b"jar");
    }

    #[test]
    fn mirror_never_used_without_a_hash() {
        let (base, _) = serve(vec![500], b"");
        let (mirror, mirror_hits) = serve(vec![200], b"jar");
        let (res, dest) = fetch(
            &format!("{base}/a.jar"),
            Some(format!("{mirror}/a.jar")),
            None,
        );
        assert!(res.is_err());
        assert_eq!(mirror_hits.load(Ordering::SeqCst), 0);
        assert!(!dest.exists());
    }

    #[test]
    fn mirror_copy_must_match_known_hash() {
        let (base, _) = serve(vec![500], b"");
        let (mirror, _) = serve(vec![200], b"jar");
        let bad = "0".repeat(40);
        let (res, dest) = fetch(
            &format!("{base}/a.jar"),
            Some(format!("{mirror}/a.jar")),
            Some(&bad),
        );
        // The primary's error is reported, and nothing unverified is left behind.
        assert!(matches!(res, Err(Error::Http(_))));
        assert!(!dest.exists());
    }

    #[test]
    fn mirror_only_for_verifiable_files() {
        let job = |url: &str, sha1: Option<&str>| DownloadJob {
            url: url.into(),
            sha1: sha1.map(String::from),
            ..job("", None)
        };
        let lib = "https://maven.minecraftforge.net/net/minecraftforge/x/1/x-1.jar";
        let installer =
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1/forge-1-installer.jar";
        assert_eq!(mirror_for(&job(lib, None)), None);
        assert!(
            mirror_for(&job(lib, Some("ab")))
                .unwrap()
                .starts_with(FORGE_MIRROR)
        );
        assert_eq!(mirror_for(&job(installer, None)), None);
        assert!(mirror_for(&job(installer, Some("ab"))).is_some());
        assert_eq!(
            mirror_for(&job("https://libraries.minecraft.net/a.jar", Some("ab"))),
            None
        );
    }

    #[test]
    fn loader_hosts_get_longer_connect_timeout() {
        assert_eq!(
            connect_timeout("https://maven.minecraftforge.net/x"),
            Some(LOADER_CONNECT_TIMEOUT)
        );
        assert_ne!(
            connect_timeout("https://piston-data.mojang.com/x"),
            Some(LOADER_CONNECT_TIMEOUT)
        );
        assert_ne!(
            connect_timeout("https://evil.test/maven.minecraftforge.net/"),
            Some(LOADER_CONNECT_TIMEOUT)
        );
    }
}
