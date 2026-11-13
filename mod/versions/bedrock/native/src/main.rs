#[cfg(windows)]
mod loader;

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use arctic_bedrock::bundle::Bundle;

    let mut args = std::env::args().skip(1);
    let pid: u32 = args
        .next()
        .ok_or("usage: arctic-bedrock-loader <PID> <BUNDLE_DIRECTORY>")?
        .parse()?;
    let dir = std::path::PathBuf::from(args.next().ok_or("a bundle directory is required")?)
        .canonicalize()?;
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    Bundle::read(&dir)?;
    let dll = dir.join("arctic_bedrock.dll").canonicalize()?;
    loader::inject(pid, &dll)?;
    println!(
        "Loaded Arctic's native adapter into process {pid}. Check arctic-bedrock-{pid}.log for renderer/JVM startup; injection alone does not confirm initialization."
    );
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The Bedrock native client requires Windows x64.");
    std::process::exit(1);
}
