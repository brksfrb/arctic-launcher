//! Read-only timing probe for the same executable fingerprint path.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: hash_profile <copied text section>")?;
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    let cng = std::env::args().nth(2).as_deref() == Some("--cng");
    let mut native = if cng {
        Some(arctic_bedrock::native_hash::Sha256::new()?)
    } else {
        None
    };
    let start = std::time::Instant::now();
    let mut bytes = 0u64;
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        if let Some(native) = &mut native {
            native.update(&buffer[..length])?;
        } else {
            hash.update(&buffer[..length]);
        }
        bytes += length as u64;
    }
    let digest = if let Some(native) = native {
        native.finish()?
    } else {
        hash.finalize().into()
    };
    println!(
        "{{\"backend\":\"{}\",\"bytes\":{bytes},\"elapsedMs\":{},\"sha256\":\"{}\"}}",
        if cng { "windows-cng" } else { "rust-sha2" },
        start.elapsed().as_millis(),
        arctic_bedrock::native_hash::hex(&digest)
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("This timing probe compares the Windows native hash backend");
}
