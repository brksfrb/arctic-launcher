use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, ERROR_SUCCESS, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    GetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{
    EqualSid, GetTokenInformation, OWNER_SECURITY_INFORMATION, PSID, SECURITY_ATTRIBUTES,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{CreateDirectoryW, FILE_ATTRIBUTE_REPARSE_POINT};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// ProgramData permits standard users to create a directory on stock Windows. Use a SID
/// for a stable per-account name and give the folder a private, inheritable ACL at creation.
pub(super) fn fallback() -> Option<PathBuf> {
    let base = PathBuf::from(std::env::var_os("ProgramData")?);
    if !base.is_absolute()
        || super::check_java_path(&base).is_err()
        || !base.is_dir()
        || std::fs::canonicalize(&base).is_ok_and(|p| super::check_java_path(&p).is_err())
    {
        return None;
    }
    match private_data_dir(&base) {
        Ok(path) => Some(path),
        Err(e) => {
            log::warn!("could not use a data folder under {}: {e}", base.display());
            None
        }
    }
}

fn private_data_dir(base: &Path) -> io::Result<PathBuf> {
    let token_user = read_token_user()?;
    // The aligned buffer owns the TOKEN_USER and the SID it points to.
    let sid = unsafe { (*token_user.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut name = null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut name) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _sid_string = LocalAllocation(name.cast());
    // ConvertSidToStringSidW returns a null-terminated string of ASCII digits and punctuation.
    let sid_string = unsafe {
        let len = (0..).take_while(|&i| *name.add(i) != 0).count();
        String::from_utf16_lossy(std::slice::from_raw_parts(name, len))
    };
    let path = base.join(format!("ArcticLauncher-{sid_string}"));
    let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // Explicit owner, with access for this user, SYSTEM and administrators only.
    let sddl: Vec<u16> =
        format!("O:{sid_string}D:P(A;OICI;FA;;;{sid_string})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)")
            .encode_utf16()
            .chain([0])
            .collect();
    let mut descriptor = null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let _descriptor = LocalAllocation(descriptor);
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    if unsafe { CreateDirectoryW(wide_path.as_ptr(), &attributes) } == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_ALREADY_EXISTS as i32) {
            return Err(error);
        }
    }
    // ProgramData is shared. Verify ownership even on a newly created folder, since
    // filesystems without ACL support may ignore the descriptor.
    let meta = std::fs::symlink_metadata(&path)?;
    if !meta.is_dir()
        || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || !owned_by(&wide_path, sid)?
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "fallback folder is not owned by this account or is a link",
        ));
    }
    tempfile::tempfile_in(&path)?;
    Ok(path)
}

fn read_token_user() -> io::Result<Vec<usize>> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut len = 0;
    // Ask the API how much space to allocate, and use pointer-aligned storage.
    unsafe { GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut len) };
    let mut buffer = vec![0usize; (len as usize).div_ceil(std::mem::size_of::<usize>())];
    if len == 0
        || unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenUser,
                buffer.as_mut_ptr().cast(),
                len,
                &mut len,
            )
        } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(buffer)
}

fn owned_by(path: &[u16], sid: PSID) -> io::Result<bool> {
    let mut owner = null_mut();
    let mut descriptor = null_mut();
    let code = unsafe {
        GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    if code != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    let _descriptor = LocalAllocation(descriptor);
    Ok(!owner.is_null() && unsafe { EqualSid(owner, sid) } != 0)
}

struct LocalAllocation(*mut c_void);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0) };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn private_fallback_is_writable_and_reused() {
        let base = tempfile::tempdir().unwrap();
        let path = super::private_data_dir(base.path()).unwrap();
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ArcticLauncher-S-")
        );
        std::fs::write(path.join("data"), b"data").unwrap();
        assert_eq!(super::private_data_dir(base.path()).unwrap(), path);
        assert_eq!(std::fs::read(path.join("data")).unwrap(), b"data");
    }
}
