//! Explicit, PID-scoped development loader. No process-wide search or automatic
//! attachment to whichever Minecraft process happens to be found first.
use std::ffi::c_void;
use std::path::Path;

use hudhook::windows::Win32::Foundation::{CloseHandle, ERROR_BAD_LENGTH, HANDLE, WAIT_OBJECT_0};
use hudhook::windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use hudhook::windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, TH32CS_SNAPMODULE,
    TH32CS_SNAPMODULE32,
};
use hudhook::windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use hudhook::windows::Win32::System::Memory::{
    MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE, VirtualAllocEx, VirtualFreeEx,
};
use hudhook::windows::Win32::System::Threading::*;
use hudhook::windows::core::{s, w};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn inject(pid: u32, dll: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(target_arch = "x86_64") {
        return Err("the native adapter requires Windows x64".into());
    }
    // SAFETY: every handle is owned by an RAII guard. The only remote memory is
    // an exactly sized UTF-16 path; it contains no executable code or credentials.
    unsafe {
        let process = Handle(OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION
                | PROCESS_CREATE_THREAD
                | PROCESS_VM_OPERATION
                | PROCESS_VM_WRITE
                | PROCESS_VM_READ,
            false,
            pid,
        )?);
        let mut name = [0u16; 32768];
        let mut size = name.len() as u32;
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            hudhook::windows::core::PWSTR(name.as_mut_ptr()),
            &mut size,
        )?;
        let path = std::path::PathBuf::from(String::from_utf16_lossy(&name[..size as usize]));
        let filename = path
            .file_name()
            .ok_or("target has no executable name")?
            .to_string_lossy();
        if !filename.eq_ignore_ascii_case("Minecraft.Windows.exe")
            && !filename.eq_ignore_ascii_case("render_host.exe")
            && !filename.eq_ignore_ascii_case("render_host_dx12.exe")
        {
            return Err(
                "target is neither Minecraft Bedrock nor Arctic's renderer test host".into(),
            );
        }
        let mut wow64 = false.into();
        IsWow64Process(process.0, &mut wow64)?;
        if wow64.as_bool() {
            return Err("cannot load an x64 mod into a 32-bit game".into());
        }
        // A second LoadLibrary increments the module count, but DllMain won't
        // start a second JVM or recreate the ready marker. Fail immediately.
        // Windows documents ERROR_BAD_LENGTH while modules load/unload. JVM
        // startup can overlap this duplicate check. Retry that error only;
        // preserve all other failures and bound the wait to two seconds.
        let snapshot_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let modules = Handle(loop {
            match CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) {
                Ok(handle) => break handle,
                Err(error)
                    if error.code()
                        == hudhook::windows::core::HRESULT::from_win32(ERROR_BAD_LENGTH.0)
                        && std::time::Instant::now() < snapshot_deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                Err(error) => return Err(error.into()),
            }
        });
        let mut entry = MODULEENTRY32W {
            dwSize: size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        Module32FirstW(modules.0, &mut entry)?;
        loop {
            let end = entry
                .szModule
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szModule.len());
            if String::from_utf16_lossy(&entry.szModule[..end])
                .eq_ignore_ascii_case("arctic_bedrock.dll")
            {
                return Err("Arctic Client is already loaded; restart the game before loading another native bundle".into());
            }
            if Module32NextW(modules.0, &mut entry).is_err() {
                break;
            }
        }
        let remote_path: Vec<u16> = dll
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let bytes = remote_path.len() * size_of::<u16>();
        let memory = VirtualAllocEx(
            process.0,
            None,
            bytes,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );
        if memory.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let mut written = 0;
            WriteProcessMemory(
                process.0,
                memory,
                remote_path.as_ptr().cast(),
                bytes,
                Some(&mut written),
            )?;
            if written != bytes {
                return Err("couldn't write the complete DLL path".into());
            }
            let load = GetProcAddress(GetModuleHandleW(w!("kernel32.dll"))?, s!("LoadLibraryW"))
                .ok_or("LoadLibraryW isn't available")?;
            let entry = std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                unsafe extern "system" fn(*mut c_void) -> u32,
            >(load);
            let thread = Handle(CreateRemoteThread(
                process.0,
                None,
                0,
                Some(entry),
                Some(memory),
                0,
                None,
            )?);
            if WaitForSingleObject(thread.0, 15_000) != WAIT_OBJECT_0 {
                // The loader may still be reading the argument. Keep it alive
                // until process exit instead of freeing a live thread's memory.
                return Err(
                    "loading timed out; the path buffer is retained until the game closes".into(),
                );
            }
            let mut exit = 0;
            GetExitCodeThread(thread.0, &mut exit)?;
            VirtualFreeEx(process.0, memory, 0, MEM_RELEASE)?;
            if exit == 0 {
                return Err("Windows couldn't load the native client DLL".into());
            }
            Ok(())
        })();
        // On error the remote thread may still exist. Retaining a path-sized
        // buffer is preferable to an asynchronous use-after-free in the game.
        result
    }
}
