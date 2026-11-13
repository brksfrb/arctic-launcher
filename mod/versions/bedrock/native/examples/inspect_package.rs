//! Run through Invoke-CommandInDesktopPackage. Reads the installed executable in
//! its package context without starting Minecraft, loading its code or accounts.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        return;
    }
    let output = std::path::Path::new(&args[2]);
    let result = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = std::fs::File::open(&args[1])?;
        let size = file.metadata()?.len();
        let mut header = [0u8; 4096];
        file.read_exact(&mut header)?;
        if &header[..2] != b"MZ" {
            return Err("not a PE executable".into());
        }
        let pe = u32::from_le_bytes(header[60..64].try_into()?) as usize;
        if pe + 24 > header.len() || &header[pe..pe + 4] != b"PE\0\0" {
            return Err("invalid PE header".into());
        }
        let count = u16::from_le_bytes(header[pe + 6..pe + 8].try_into()?) as usize;
        let optional = u16::from_le_bytes(header[pe + 20..pe + 22].try_into()?) as usize;
        let mut sections = Vec::new();
        for index in 0..count {
            let off = pe + 24 + optional + index * 40;
            let h = header.get(off..off + 40).ok_or("invalid section table")?;
            let length = u32::from_le_bytes(h[16..20].try_into()?) as usize;
            let start = u32::from_le_bytes(h[20..24].try_into()?) as u64;
            let rva = u32::from_le_bytes(h[12..16].try_into()?);
            let flags = u32::from_le_bytes(h[36..40].try_into()?);
            if start + length as u64 > size || length > 512 * 1024 * 1024 {
                return Err("invalid section bounds".into());
            }
            let name = String::from_utf8_lossy(&h[..8])
                .trim_end_matches('\0')
                .to_owned();
            if flags & 0x20000000 != 0
                || matches!(name.as_str(), ".rdata" | ".reloc" | ".data" | ".pdata")
            {
                let path = output.with_extension(format!("{rva:x}.bin"));
                let mut destination = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)?;
                file.seek(SeekFrom::Start(start))?;
                if std::io::copy(&mut (&mut file).take(length as u64), &mut destination)?
                    != length as u64
                {
                    return Err("truncated section".into());
                }
            }
            sections.push(
                serde_json::json!({"name":name,"rva":rva,"size":length,"flags":flags,
                "virtualSize":u32::from_le_bytes(h[8..12].try_into()?)}),
            );
        }
        Ok(
            serde_json::json!({"readable":true,"size":size,"sections":sections,"gameStarted":false,
            "machine":u16::from_le_bytes(header[pe+4..pe+6].try_into()?),
            "timestamp":u32::from_le_bytes(header[pe+8..pe+12].try_into()?),
            "preferredBase":u64::from_le_bytes(header[pe+24+24..pe+24+32].try_into()?),
            "relocationRva":u32::from_le_bytes(header[pe+24+152..pe+24+156].try_into()?),
            "relocationSize":u32::from_le_bytes(header[pe+24+156..pe+24+160].try_into()?),
            "exceptionRva":u32::from_le_bytes(header[pe+24+136..pe+24+140].try_into()?),
            "exceptionSize":u32::from_le_bytes(header[pe+24+140..pe+24+144].try_into()?),
            "imageSize":u32::from_le_bytes(header[pe+24+56..pe+24+60].try_into()?)}),
        )
    })();
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            serde_json::json!({"readable":false,"error":error.to_string(),"gameStarted":false})
        }
    };
    let _ = std::fs::write(output, report.to_string());
}
