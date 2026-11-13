//! Seekable COM stream over the already-created, inherited output handle.
//! No filename is opened, removed or replaced by the encoder child.
use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
};
use windows::{
    Win32::{
        Foundation::{
            E_FAIL, E_NOTIMPL, E_POINTER, S_OK, STG_E_ACCESSDENIED, STG_E_INVALIDFUNCTION,
            STG_E_WRITEFAULT,
        },
        System::Com::{
            ISequentialStream_Impl, IStream, IStream_Impl, LOCKTYPE, STATFLAG, STATSTG, STGC,
            STGM_WRITE, STGTY_STREAM, STREAM_SEEK, STREAM_SEEK_CUR, STREAM_SEEK_END,
            STREAM_SEEK_SET,
        },
    },
    core::{Error, HRESULT, Ref, Result},
};

#[windows::core::implement(IStream)]
struct OutputStream {
    file: Arc<Mutex<File>>,
    position: Mutex<u64>,
}
fn failure(error: std::io::Error) -> Error {
    Error::from_hresult(
        error
            .raw_os_error()
            .map_or(E_FAIL, |code| HRESULT::from_win32(code as u32)),
    )
}
pub(super) fn open(mut file: File) -> Result<IStream> {
    let metadata = file.metadata().map_err(failure)?;
    if !metadata.is_file() || metadata.len() != 0 {
        return Err(STG_E_INVALIDFUNCTION.into());
    }
    file.seek(SeekFrom::Start(0)).map_err(failure)?;
    Ok(OutputStream {
        file: Arc::new(Mutex::new(file)),
        position: Mutex::new(0),
    }
    .into())
}
impl ISequentialStream_Impl for OutputStream_Impl {
    fn Read(&self, _: *mut core::ffi::c_void, count: u32, read: *mut u32) -> HRESULT {
        if !read.is_null() {
            unsafe {
                read.write(0);
            }
        }
        if count == 0 { S_OK } else { STG_E_ACCESSDENIED }
    }
    fn Write(&self, bytes: *const core::ffi::c_void, count: u32, written: *mut u32) -> HRESULT {
        if !written.is_null() {
            unsafe {
                written.write(0);
            }
        }
        if count == 0 {
            return S_OK;
        }
        if bytes.is_null() {
            return E_POINTER;
        }
        let Ok(mut position) = self.position.lock() else {
            return E_FAIL;
        };
        let Ok(mut file) = self.file.lock() else {
            return E_FAIL;
        };
        if position
            .checked_add(u64::from(count))
            .is_none_or(|end| end > i64::MAX as u64)
        {
            return STG_E_INVALIDFUNCTION;
        }
        if let Err(error) = file.seek(SeekFrom::Start(*position)) {
            return failure(error).code();
        }
        // COM supplies a readable buffer for this call only. Its bytes are
        // copied synchronously and no foreign pointer is retained.
        let bytes = unsafe { std::slice::from_raw_parts(bytes.cast::<u8>(), count as usize) };
        let mut copied = 0;
        while copied < bytes.len() {
            match file.write(&bytes[copied..]) {
                Ok(0) => return STG_E_WRITEFAULT,
                Ok(length) => {
                    copied += length;
                    *position += length as u64;
                    if !written.is_null() {
                        unsafe {
                            written.write(copied as u32);
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return failure(error).code(),
            }
        }
        S_OK
    }
}
impl IStream_Impl for OutputStream_Impl {
    fn Seek(&self, offset: i64, origin: STREAM_SEEK, result: *mut u64) -> Result<()> {
        let mut position = self
            .position
            .lock()
            .map_err(|_| Error::from_hresult(E_FAIL))?;
        let base = match origin {
            STREAM_SEEK_SET => 0,
            STREAM_SEEK_CUR => *position,
            STREAM_SEEK_END => self
                .file
                .lock()
                .map_err(|_| Error::from_hresult(E_FAIL))?
                .metadata()
                .map_err(failure)?
                .len(),
            _ => return Err(STG_E_INVALIDFUNCTION.into()),
        };
        let next = i128::from(base) + i128::from(offset);
        *position = u64::try_from(next)
            .ok()
            .filter(|value| *value <= i64::MAX as u64)
            .ok_or_else(|| Error::from_hresult(STG_E_INVALIDFUNCTION))?;
        if !result.is_null() {
            unsafe {
                result.write(*position);
            }
        }
        Ok(())
    }
    fn SetSize(&self, size: u64) -> Result<()> {
        if size > i64::MAX as u64 {
            return Err(STG_E_INVALIDFUNCTION.into());
        }
        self.file
            .lock()
            .map_err(|_| Error::from_hresult(E_FAIL))?
            .set_len(size)
            .map_err(failure)
    }
    fn CopyTo(&self, _: Ref<IStream>, _: u64, read: *mut u64, written: *mut u64) -> Result<()> {
        if !read.is_null() {
            unsafe {
                read.write(0);
            }
        }
        if !written.is_null() {
            unsafe {
                written.write(0);
            }
        }
        Err(E_NOTIMPL.into())
    }
    fn Commit(&self, _: &STGC) -> Result<()> {
        self.file
            .lock()
            .map_err(|_| Error::from_hresult(E_FAIL))?
            .sync_all()
            .map_err(failure)
    }
    fn Revert(&self) -> Result<()> {
        Err(STG_E_INVALIDFUNCTION.into())
    }
    fn LockRegion(&self, _: u64, _: u64, _: &LOCKTYPE) -> Result<()> {
        Err(STG_E_INVALIDFUNCTION.into())
    }
    fn UnlockRegion(&self, _: u64, _: u64, _: u32) -> Result<()> {
        Err(STG_E_INVALIDFUNCTION.into())
    }
    fn Stat(&self, output: *mut STATSTG, _: &STATFLAG) -> Result<()> {
        if output.is_null() {
            return Err(E_POINTER.into());
        }
        let size = self
            .file
            .lock()
            .map_err(|_| Error::from_hresult(E_FAIL))?
            .metadata()
            .map_err(failure)?
            .len();
        let mut stat: STATSTG = unsafe { std::mem::zeroed() };
        stat.r#type = STGTY_STREAM.0 as u32;
        stat.cbSize = size;
        stat.grfMode = STGM_WRITE;
        unsafe {
            output.write(stat);
        }
        Ok(())
    }
    fn Clone(&self) -> Result<IStream> {
        let position = *self
            .position
            .lock()
            .map_err(|_| Error::from_hresult(E_FAIL))?;
        // The file is shared but each COM clone has an independent cursor.
        Ok(OutputStream {
            file: self.file.clone(),
            position: Mutex::new(position),
        }
        .into())
    }
}
