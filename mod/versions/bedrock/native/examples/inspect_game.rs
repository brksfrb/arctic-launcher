//! Read-only native binding inspection. The target's primary thread is never
//! resumed, so this tool cannot open a game window or initialize an account.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use hudhook::windows::Win32::Foundation::{CloseHandle, HANDLE};
    use hudhook::windows::Win32::System::Diagnostics::Debug::{
        DebugActiveProcessStop, ReadProcessMemory,
    };
    use hudhook::windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use hudhook::windows::Win32::System::Threading::*;
    use hudhook::windows::core::{HSTRING, PCWSTR, s, w};
    use std::ffi::c_void;

    struct Suspended(PROCESS_INFORMATION, bool);
    impl Drop for Suspended {
        fn drop(&mut self) {
            unsafe {
                let _ = TerminateProcess(self.0.hProcess, 0);
                if self.1 {
                    let _ = DebugActiveProcessStop(self.0.dwProcessId);
                }
                let _ = WaitForSingleObject(self.0.hProcess, 5000);
                let _ = CloseHandle(self.0.hThread);
                let _ = CloseHandle(self.0.hProcess);
            }
        }
    }
    #[repr(C)]
    #[derive(Default)]
    struct BasicInfo {
        reserved: usize,
        peb: usize,
        reserved2: [usize; 2],
        pid: usize,
        parent: usize,
    }
    type Query = unsafe extern "system" fn(HANDLE, u32, *mut c_void, u32, *mut u32) -> i32;
    unsafe fn read(
        handle: HANDLE,
        address: usize,
        bytes: &mut [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut count = 0;
        unsafe {
            ReadProcessMemory(
                handle,
                address as _,
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                Some(&mut count),
            )?;
        }
        if count != bytes.len() {
            return Err("incomplete image read".into());
        }
        Ok(())
    }
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: inspect_game <Minecraft.Windows.exe> <output text section>")?;
    let out = std::env::args().nth(2).ok_or("output path required")?;
    let debug = std::env::args().nth(3).as_deref() == Some("--debug");
    let app = HSTRING::from(path);
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR(app.as_ptr()),
            None,
            None,
            None,
            false,
            CREATE_SUSPENDED
                | CREATE_NO_WINDOW
                | if debug {
                    DEBUG_ONLY_THIS_PROCESS
                } else {
                    PROCESS_CREATION_FLAGS(0)
                },
            None,
            None,
            &startup,
            &mut process,
        )?;
        let process = Suspended(process, debug);
        let mut image_path = [0u16; 32768];
        let mut path_len = image_path.len() as u32;
        QueryFullProcessImageNameW(
            process.0.hProcess,
            PROCESS_NAME_WIN32,
            hudhook::windows::core::PWSTR(image_path.as_mut_ptr()),
            &mut path_len,
        )?;
        let actual_path = String::from_utf16_lossy(&image_path[..path_len as usize]);
        println!("Suspended process image: {actual_path}");
        if !std::path::Path::new(&actual_path)
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("Minecraft.Windows.exe"))
        {
            return Err(
                "launch redirected to a helper; no game image is available for inspection".into(),
            );
        }
        let proc = GetProcAddress(
            GetModuleHandleW(w!("ntdll.dll"))?,
            s!("NtQueryInformationProcess"),
        )
        .ok_or("NtQueryInformationProcess unavailable")?;
        let query: Query = std::mem::transmute(proc);
        let mut info = BasicInfo::default();
        let status = query(
            process.0.hProcess,
            0,
            (&mut info as *mut BasicInfo).cast(),
            size_of::<BasicInfo>() as u32,
            std::ptr::null_mut(),
        );
        if status < 0 {
            return Err(format!("process metadata read failed: {status:#x}").into());
        }
        let mut base = [0u8; 8];
        read(process.0.hProcess, info.peb + 0x10, &mut base)?;
        let base = u64::from_le_bytes(base) as usize;
        let mut header = [0u8; 4096];
        read(process.0.hProcess, base, &mut header)?;
        if header[..2] != *b"MZ" {
            return Err("target image has no DOS header".into());
        }
        let pe = u32::from_le_bytes(header[0x3c..0x40].try_into()?) as usize;
        if pe + 24 > header.len() {
            return Err("invalid PE header offset".into());
        }
        let sections = u16::from_le_bytes(header[pe + 6..pe + 8].try_into()?) as usize;
        let optional = u16::from_le_bytes(header[pe + 20..pe + 22].try_into()?) as usize;
        let mut dumped = false;
        for i in 0..sections {
            let off = pe + 24 + optional + i * 40;
            let section = header
                .get(off..off + 40)
                .ok_or("invalid PE section header")?;
            let name = String::from_utf8_lossy(&section[..8]);
            let flags = u32::from_le_bytes(section[36..40].try_into()?);
            let size = u32::from_le_bytes(section[16..20].try_into()?) as usize;
            let rva = u32::from_le_bytes(section[12..16].try_into()?) as usize;
            println!(
                "Section {} RVA {rva:#x}, raw size {size}, flags {flags:#x}",
                name.trim_end_matches('\0')
            );
            if flags & 0x20000000 == 0 {
                continue;
            }
            if size > 512 * 1024 * 1024 {
                return Err("unexpected executable section size".into());
            }
            let mut text = vec![0; size];
            read(process.0.hProcess, base + rva, &mut text)?;
            let output = if dumped {
                format!("{out}.{rva:x}")
            } else {
                out.clone()
            };
            std::fs::write(output, text)?;
            dumped = true;
        }
        if !dumped {
            return Err("target image has no executable section".into());
        }
        println!("Read suspended image. No target code executed.");
        Ok(())
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This read-only native image probe requires Windows x64.");
}
