use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::mem::size_of;
use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
use std::path::{Path, PathBuf};
use std::ptr::from_mut;
use std::sync::mpsc::{self, Receiver};

use arctic_core::storage::DataDirs;
use eframe::egui;
use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE, TRUST_E_NOSIGNATURE};
use windows_sys::Win32::Security::Cryptography::{
    CERT_SHA256_HASH_PROP_ID, CRYPT_INTEGER_BLOB, CertCompareCertificateName,
    CertGetCertificateContextProperty, X509_ASN_ENCODING,
};
use windows_sys::Win32::Security::WinTrust::{
    WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
    WTD_CHOICE_FILE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
    WTHelperGetProvCertFromChain, WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData,
    WinVerifyTrust,
};

use super::{Banner, Verification, banner};

// DER-encoded subject from the v0.6.6 signer. See CERTUM.md for renewal instructions.
const PUBLISHER_SUBJECT: &[u8] = &[
    0x30, 0x6b, 0x31, 0x0b, 0x30, 0x09, 0x06, 0x03, 0x55, 0x04, 0x06, 0x13, 0x02, 0x54, 0x52, 0x31,
    0x0f, 0x30, 0x0d, 0x06, 0x03, 0x55, 0x04, 0x07, 0x0c, 0x06, 0x53, 0x61, 0x6d, 0x73, 0x75, 0x6e,
    0x31, 0x1e, 0x30, 0x1c, 0x06, 0x03, 0x55, 0x04, 0x0a, 0x0c, 0x15, 0x4f, 0x70, 0x65, 0x6e, 0x20,
    0x53, 0x6f, 0x75, 0x72, 0x63, 0x65, 0x20, 0x44, 0x65, 0x76, 0x65, 0x6c, 0x6f, 0x70, 0x65, 0x72,
    0x31, 0x2b, 0x30, 0x29, 0x06, 0x03, 0x55, 0x04, 0x03, 0x0c, 0x22, 0x4f, 0x70, 0x65, 0x6e, 0x20,
    0x53, 0x6f, 0x75, 0x72, 0x63, 0x65, 0x20, 0x44, 0x65, 0x76, 0x65, 0x6c, 0x6f, 0x70, 0x65, 0x72,
    0x20, 0x4c, 0x65, 0x79, 0x6c, 0x61, 0x20, 0x42, 0x61, 0x72, 0x6c, 0x61, 0x6b,
];
// SHA-256 fingerprint of the Certum Code Signing 2021 CA certificate.
const ISSUER_SHA256: [u8; 32] = [
    0x5a, 0xc8, 0x2c, 0xbe, 0x9f, 0x28, 0x35, 0x1e, 0x85, 0xd5, 0x26, 0x22, 0x93, 0xbc, 0xfc, 0x8b,
    0xac, 0xab, 0xea, 0xd1, 0x29, 0x4f, 0x24, 0x8c, 0x1d, 0xf1, 0x7f, 0x81, 0xce, 0x5a, 0xc3, 0xce,
];

pub(crate) struct BuildSignature {
    pending: Option<Receiver<(Verification, bool)>>,
    verification: Option<Verification>,
    dismissed: bool,
    release: bool,
    dismissal_path: PathBuf,
}

impl BuildSignature {
    pub(crate) fn start(root: &DataDirs, ctx: &egui::Context) -> Self {
        let dismissal_path = root.root().join("unsigned-build-warning-dismissed");
        let marker = dismissal_path.clone();
        let ctx = ctx.clone();
        let (send, pending) = mpsc::channel();
        if let Err(error) = std::thread::Builder::new()
            .name("build-signature".into())
            .spawn(move || {
                let verification = match verify_running_exe() {
                    Ok(verification) => verification,
                    Err(error) => {
                        log::warn!("Could not access launcher executable for signature verification: {error}");
                        return;
                    }
                };
                let dismissed = verification == Verification::Unsigned && marker.is_file();
                if send.send((verification, dismissed)).is_ok() {
                    ctx.request_repaint();
                }
            })
        {
            log::warn!("Could not start launcher signature check: {error}");
        }
        Self {
            pending: Some(pending),
            verification: None,
            dismissed: false,
            release: arctic_core::update::is_release_build(),
            dismissal_path,
        }
    }

    pub(crate) fn poll(&mut self) {
        let Some(pending) = &self.pending else {
            return;
        };
        match pending.try_recv() {
            Ok((verification, dismissed)) => {
                self.verification = Some(verification);
                self.dismissed = dismissed;
                self.pending = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => self.pending = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    pub(crate) fn is_unsigned(&self) -> bool {
        self.verification == Some(Verification::Unsigned)
    }

    pub(crate) fn banner(&self) -> Option<Banner> {
        banner(self.verification, self.release, self.dismissed)
    }

    pub(crate) fn dismiss_unsigned(&mut self) {
        self.dismissed = true;
        if let Err(error) = fs::write(&self.dismissal_path, []) {
            log::warn!("Could not save unsigned build warning dismissal: {error}");
        }
    }
}

fn verify_running_exe() -> io::Result<Verification> {
    std::env::current_exe().and_then(|exe| verify_exe(&exe))
}

fn verify_exe(exe: &Path) -> io::Result<Verification> {
    let mut handle = File::open(exe)?;
    let path: Vec<u16> = exe.as_os_str().encode_wide().chain([0]).collect();
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: path.as_ptr(),
        hFile: handle.as_raw_handle(),
        ..Default::default()
    };
    let mut trust = WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut file },
        dwStateAction: WTD_STATEACTION_VERIFY,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // The path, file handle and trust input stay alive through VERIFY and CLOSE.
    let (status, official) = unsafe {
        let status = WinVerifyTrust(
            INVALID_HANDLE_VALUE,
            &mut action,
            from_mut(&mut trust).cast(),
        );
        let official = status == 0 && matches_publisher(trust.hWVTStateData);
        trust.dwStateAction = WTD_STATEACTION_CLOSE;
        WinVerifyTrust(
            INVALID_HANDLE_VALUE,
            &mut action,
            from_mut(&mut trust).cast(),
        );
        (status, official)
    };

    if official {
        Ok(Verification::Official)
    } else if status == TRUST_E_NOSIGNATURE
        && has_no_certificate_table(&mut handle).unwrap_or(false)
    {
        Ok(Verification::Unsigned)
    } else {
        Ok(Verification::Untrusted)
    }
}

// Only called while the successfully verified WinVerifyTrust state is still open.
unsafe fn matches_publisher(state: HANDLE) -> bool {
    // The provider owns all returned pointers. Inspect only the primary signer's
    // verified chain, where index 0 is the leaf and index 1 is its immediate issuer.
    unsafe {
        let provider = WTHelperProvDataFromStateData(state);
        if provider.is_null() {
            return false;
        }
        let signer = WTHelperGetProvSignerFromChain(provider, 0, 0, 0);
        let Some(signer_info) = signer.as_ref() else {
            return false;
        };
        if signer_info.csCertChain < 2 {
            return false;
        }
        let leaf = WTHelperGetProvCertFromChain(signer, 0);
        let issuer = WTHelperGetProvCertFromChain(signer, 1);
        let (Some(leaf), Some(issuer)) = (leaf.as_ref(), issuer.as_ref()) else {
            return false;
        };
        let Some(leaf_info) = leaf.pCert.as_ref().and_then(|cert| cert.pCertInfo.as_ref()) else {
            return false;
        };
        let subject = CRYPT_INTEGER_BLOB {
            cbData: PUBLISHER_SUBJECT.len() as u32,
            pbData: PUBLISHER_SUBJECT.as_ptr().cast_mut(),
        };
        if CertCompareCertificateName(X509_ASN_ENCODING, &leaf_info.Subject, &subject) == 0
            || issuer.pCert.is_null()
        {
            return false;
        }

        let mut fingerprint = [0u8; 32];
        let mut size = fingerprint.len() as u32;
        CertGetCertificateContextProperty(
            issuer.pCert,
            CERT_SHA256_HASH_PROP_ID,
            fingerprint.as_mut_ptr().cast(),
            &mut size,
        ) != 0
            && size as usize == fingerprint.len()
            && fingerprint == ISSUER_SHA256
    }
}

// WinVerifyTrust can report NOSIGNATURE for malformed signatures. Only a PE
// with an empty security directory qualifies for the quieter unsigned state.
fn has_no_certificate_table(file: &mut File) -> io::Result<bool> {
    let mut dos = [0; 64];
    file.rewind()?;
    file.read_exact(&mut dos)?;
    if &dos[..2] != b"MZ" {
        return Ok(false);
    }
    let pe_offset = u32::from_le_bytes(dos[60..64].try_into().unwrap());
    file.seek(SeekFrom::Start(u64::from(pe_offset)))?;
    let mut pe = [0; 26];
    file.read_exact(&mut pe)?;
    if &pe[..4] != b"PE\0\0" {
        return Ok(false);
    }
    let directory_offset = match u16::from_le_bytes([pe[24], pe[25]]) {
        0x10b => 96,  // PE32
        0x20b => 112, // PE32+
        _ => return Ok(false),
    };
    let security_offset = directory_offset + 4 * 8;
    let optional_size = u16::from_le_bytes([pe[20], pe[21]]);
    if optional_size < security_offset + 8 {
        return Ok(false);
    }
    let directories = u64::from(pe_offset) + 24 + u64::from(directory_offset);
    file.seek(SeekFrom::Start(directories - 4))?;
    let mut count = [0; 4];
    file.read_exact(&mut count)?;
    if u32::from_le_bytes(count) <= 4 {
        return Ok(false);
    }
    file.seek(SeekFrom::Start(directories + 4 * 8))?;
    let mut entry = [0; 8];
    file.read_exact(&mut entry)?;
    Ok(entry == [0; 8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticode_reports_unsigned_test_executable() {
        assert_eq!(verify_running_exe().unwrap(), Verification::Unsigned);
    }

    #[test]
    #[ignore = "requires the published signed Windows launcher as a fixture"]
    fn authenticode_accepts_the_official_publisher_and_rejects_tampering() {
        let fixture = PathBuf::from(std::env::var_os("ARCTIC_SIGNED_EXE").unwrap());
        assert_eq!(verify_exe(&fixture).unwrap(), Verification::Official);

        let dir = tempfile::tempdir().unwrap();
        let tampered = dir.path().join("tampered.exe");
        let mut bytes = fs::read(fixture).unwrap();
        // The DOS stub is covered by the Authenticode digest.
        bytes[0x40] ^= 1;
        fs::write(&tampered, bytes).unwrap();
        assert_eq!(verify_exe(&tampered).unwrap(), Verification::Untrusted);
    }
}
