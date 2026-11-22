#[cfg(windows)]
mod loader;

/// Appends to `arctic-bedrock-loader.log` in the bundle (and to stderr), so a
/// failed load leaves a trace even when nothing reaches the game.
#[cfg(windows)]
struct LoaderLog(Option<std::path::PathBuf>);

#[cfg(windows)]
impl LoaderLog {
    fn line(&self, message: impl AsRef<str>) {
        use std::io::Write;
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let text = format!(
            "[{secs}] [loader {}] {}",
            std::process::id(),
            message.as_ref()
        );
        eprintln!("{text}");
        if let Some(path) = &self.0
            && let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
        {
            let _ = writeln!(file, "{text}");
        }
    }
}

/// `\\?\C:\x` → `C:\x`: the game resolves a plain DLL path the usual way.
#[cfg(windows)]
fn plain(path: std::path::PathBuf) -> std::path::PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => rest.into(),
        _ => path,
    }
}

#[cfg(windows)]
fn main() {
    let mut args = std::env::args().skip(1);
    let pid = args.next();
    let dir = args.next().map(std::path::PathBuf::from);
    let log = LoaderLog(dir.as_ref().map(|d| d.join("arctic-bedrock-loader.log")));
    log.line(format!(
        "started: pid={} bundle={}",
        pid.as_deref().unwrap_or("(none)"),
        dir.as_ref()
            .map_or("(none)".into(), |d| d.display().to_string())
    ));
    let result = (|| -> Result<u32, Box<dyn std::error::Error>> {
        use arctic_bedrock::bundle::Bundle;
        let pid: u32 = pid
            .ok_or("usage: arctic-bedrock-loader <PID> <BUNDLE_DIRECTORY>")?
            .parse()?;
        let dir = plain(
            dir.ok_or("a bundle directory is required")?
                .canonicalize()?,
        );
        if args.next().is_some() {
            return Err("unexpected argument".into());
        }
        Bundle::read(&dir).map_err(|e| format!("bundle config: {e}"))?;
        let dll = plain(dir.join("arctic_bedrock.dll").canonicalize()?);
        log.line(format!("injecting {} into process {pid}", dll.display()));
        loader::inject(pid, &dll, &|step| log.line(step))?;
        Ok(pid)
    })();
    match result {
        Ok(pid) => {
            log.line(format!("LoadLibraryW succeeded in process {pid}"));
            println!(
                "Loaded Arctic's native adapter into process {pid}. Check arctic-bedrock-{pid}.log for renderer/JVM startup; injection alone does not confirm initialization."
            );
        }
        Err(e) => {
            log.line(format!("failed: {e}"));
            std::process::exit(1);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The Bedrock native client requires Windows x64.");
    std::process::exit(1);
}
