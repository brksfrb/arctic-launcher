use hudhook::windows::Win32::Foundation::HANDLE;
use hudhook::windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use hudhook::windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use std::ffi::c_void;
/// Read only x64 process-parameter DesktopInfo when a thread has not attached
/// to User32. Command lines, environment blocks and application data are omitted.
pub(crate) unsafe fn startup_desktop(process: HANDLE) -> Option<String> {
    use hudhook::windows::core::{s, w};
    #[repr(C)]
    #[derive(Default)]
    struct Basic {
        reserved: usize,
        peb: usize,
        reserved2: [usize; 2],
        pid: usize,
        parent: usize,
    }
    type Query = unsafe extern "system" fn(HANDLE, u32, *mut c_void, u32, *mut u32) -> i32;
    unsafe fn copy<const N: usize>(process: HANDLE, at: usize) -> Option<[u8; N]> {
        let mut bytes = [0u8; N];
        let mut size = 0;
        unsafe {
            ReadProcessMemory(
                process,
                at as *const c_void,
                bytes.as_mut_ptr().cast(),
                N,
                Some(&mut size),
            )
        }
        .ok()?;
        (size == N).then_some(bytes)
    }
    let proc = unsafe {
        GetProcAddress(
            GetModuleHandleW(w!("ntdll.dll")).ok()?,
            s!("NtQueryInformationProcess"),
        )
    }?;
    let query: Query = unsafe { std::mem::transmute(proc) };
    let mut basic = Basic::default();
    if unsafe {
        query(
            process,
            0,
            (&mut basic as *mut Basic).cast(),
            size_of::<Basic>() as u32,
            std::ptr::null_mut(),
        )
    } < 0
    {
        return None;
    }
    let params =
        u64::from_le_bytes(unsafe { copy::<8>(process, basic.peb.checked_add(0x20)?) }?) as usize;
    let header = unsafe { copy::<16>(process, params) }?;
    let length = u32::from_le_bytes(header[4..8].try_into().ok()?);
    let flags = u32::from_le_bytes(header[8..12].try_into().ok()?);
    if length < 0xd0 || flags & 1 == 0 {
        return None;
    }
    // RTL_USER_PROCESS_PARAMETERS::DesktopInfo (phnt/ntrtl.h), x64.
    let descriptor = unsafe { copy::<16>(process, params.checked_add(0xc0)?) }?;
    let size = u16::from_le_bytes(descriptor[..2].try_into().ok()?) as usize;
    let capacity = u16::from_le_bytes(descriptor[2..4].try_into().ok()?) as usize;
    let source = u64::from_le_bytes(descriptor[8..16].try_into().ok()?) as usize;
    if size == 0 || size > 512 || !size.is_multiple_of(2) || capacity < size || source < 0x10000 {
        return None;
    }
    let mut bytes = vec![0u8; size];
    let mut copied = 0;
    unsafe {
        ReadProcessMemory(
            process,
            source as *const c_void,
            bytes.as_mut_ptr().cast(),
            size,
            Some(&mut copied),
        )
    }
    .ok()?;
    if copied != size {
        return None;
    }
    String::from_utf16(
        &bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b))
            .collect::<Vec<_>>(),
    )
    .ok()
}
