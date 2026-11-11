//! Optional probe reflection through an already installed Windows SDK compiler.
//! Resolve dynamically so ordinary launches never depend on a DXC installation.
use hudhook::windows::Win32::Graphics::Direct3D::Dxc::{CLSID_DxcUtils, DxcBuffer, IDxcUtils};
use hudhook::windows::Win32::Graphics::Direct3D12::ID3D12ShaderReflection;
use hudhook::windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
    LoadLibraryExW,
};
use hudhook::windows::core::{GUID, HRESULT, Interface, PCWSTR, s};
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::OnceLock;

type Create = unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;
static FACTORY: OnceLock<Result<Create, String>> = OnceLock::new();
fn factory() -> Result<Create, String> {
    FACTORY
        .get_or_init(|| {
            let root = PathBuf::from(
                std::env::var_os("ProgramFiles(x86)").ok_or("Windows SDK location unavailable")?,
            )
            .join("Windows Kits/10/bin");
            let mut versions = Vec::new();
            for entry in std::fs::read_dir(root)
                .map_err(|e| e.to_string())?
                .take(64)
                .flatten()
            {
                let version = entry
                    .file_name()
                    .to_string_lossy()
                    .split('.')
                    .map(str::parse::<u32>)
                    .collect::<Result<Vec<_>, _>>();
                if let Ok(v) = version
                    && v.len() == 4
                {
                    let path = entry.path().join("x64/dxcompiler.dll");
                    if path.is_file() {
                        versions.push((v, path));
                    }
                }
            }
            versions.sort_by(|a, b| b.0.cmp(&a.0));
            let path = &versions
                .first()
                .ok_or("no installed x64 Windows SDK DXC compiler")?
                .1;
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            // The module reference intentionally remains loaded while its factory and
            // reflection interfaces can exist. No DLL is downloaded or bundled.
            let module = unsafe {
                LoadLibraryExW(
                    PCWSTR(wide.as_ptr()),
                    None,
                    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                )
            }
            .map_err(|e| e.to_string())?;
            let entry = unsafe { GetProcAddress(module, s!("DxcCreateInstance")) }
                .ok_or("DXC factory unavailable")?;
            Ok(unsafe { std::mem::transmute::<usize, Create>(entry as *const () as usize) })
        })
        .clone()
}
pub(super) fn reflection(bytes: &[u8]) -> Result<ID3D12ShaderReflection, String> {
    let create = factory()?;
    let mut raw = std::ptr::null_mut();
    unsafe { create(&CLSID_DxcUtils, &IDxcUtils::IID, &mut raw) }
        .ok()
        .map_err(|e| e.to_string())?;
    if raw.is_null() {
        return Err("DXC returned null utilities".into());
    }
    let utils = unsafe { IDxcUtils::from_raw(raw) };
    let buffer = DxcBuffer {
        Ptr: bytes.as_ptr() as _,
        Size: bytes.len(),
        Encoding: 0,
    };
    let mut raw = std::ptr::null_mut();
    unsafe { utils.CreateReflection(&buffer, &ID3D12ShaderReflection::IID, &mut raw) }
        .map_err(|e| e.to_string())?;
    if raw.is_null() {
        return Err("DXC returned null reflection".into());
    }
    Ok(unsafe { ID3D12ShaderReflection::from_raw(raw) })
}
