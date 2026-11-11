//! Read-only public D3D12 shader/root-signature metadata, attached to COM lifetime.
//! Reflection describes declarations, not GPU execution or deterministic rendering.
use hudhook::windows::Win32::Graphics::Direct3D::Fxc::D3DReflect;
use hudhook::windows::Win32::Graphics::Direct3D::{D3D_CT_CBUFFER, D3D_SIT_CBUFFER};
use hudhook::windows::Win32::Graphics::Direct3D12::*;
use hudhook::windows::core::{GUID, IUnknown, Interface, PCSTR};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::{CStr, c_void};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};

const SHADERS: GUID = GUID::from_u128(0x1435ec66_4536_42b6_a5ca_42da9f16b6de);
const ROOT: GUID = GUID::from_u128(0xbbd1700e_44e1_4f07_908c_bfba31ff99bd);
#[path = "shader_dxc.rs"]
mod dxc;
const MAX_SHADER_BYTES: usize = 4 * 1024 * 1024;
const MAX_LAYOUT_BYTES: usize = 8192;
static CACHE: OnceLock<Mutex<HashMap<[u8; 32], Value>>> = OnceLock::new();
static PIPELINES: AtomicU64 = AtomicU64::new(0);
static ROOTS: AtomicU64 = AtomicU64::new(0);
static KNOWN_ROOTS: AtomicU64 = AtomicU64::new(0);
static OMITTED: AtomicU64 = AtomicU64::new(0);
static ROOT_PROFILES: Mutex<Vec<Value>> = Mutex::new(Vec::new());
fn root_profile(profile: Value) {
    if crate::native_clock::probe_enabled()
        && let Ok(mut profiles) = ROOT_PROFILES.lock()
        && profiles.len() < 16
        && !profiles.contains(&profile)
    {
        profiles.push(profile);
    }
}
pub(crate) type ShaderPair = ([u8; 32], [u8; 32]);
/// Root parameters 2/3: vertex register/space, pixel register/space.
pub(crate) type RootBinding = [u32; 4];
fn cache() -> &'static Mutex<HashMap<[u8; 32], Value>> {
    CACHE.get_or_init(Mutex::default)
}
fn name(value: PCSTR) -> Result<String, String> {
    if value.is_null() {
        return Err("reflection name missing".into());
    }
    // Reflection owns these documented NUL-terminated strings until its owner drops.
    let text = unsafe { CStr::from_ptr(value.0.cast()) }
        .to_str()
        .map_err(|_| "invalid reflection name")?;
    if text.is_empty() || text.len() > 96 || text.chars().any(char::is_control) {
        return Err("reflection name exceeds bound".into());
    }
    Ok(text.into())
}
fn reflect(bytes: &[u8]) -> Result<Value, String> {
    let mut raw = std::ptr::null_mut();
    let classic = unsafe {
        D3DReflect(
            bytes.as_ptr() as _,
            bytes.len(),
            &ID3D12ShaderReflection::IID,
            &mut raw,
        )
    };
    let (shader, backend) = if classic.is_ok() && !raw.is_null() {
        (unsafe { ID3D12ShaderReflection::from_raw(raw) }, "fxc")
    } else if bytes.starts_with(b"DXBC") {
        (
            dxc::reflection(bytes).map_err(|e| format!("DXIL reflection: {e}"))?,
            "dxc",
        )
    } else {
        return Err("unsupported compiled shader container".into());
    };
    let mut desc = D3D12_SHADER_DESC::default();
    unsafe { shader.GetDesc(&mut desc) }.map_err(|e| e.to_string())?;
    if desc.ConstantBuffers > 4 || desc.BoundResources > 128 {
        return Err("shader reflection exceeds buffer/resource bound".into());
    }
    let mut bindings = HashMap::new();
    for index in 0..desc.BoundResources {
        let mut b = D3D12_SHADER_INPUT_BIND_DESC::default();
        unsafe { shader.GetResourceBindingDesc(index, &mut b) }.map_err(|e| e.to_string())?;
        if b.Type == D3D_SIT_CBUFFER {
            let key = name(b.Name)?;
            if b.BindCount != 1 || bindings.insert(key, (b.BindPoint, b.Space)).is_some() {
                return Err("ambiguous constant buffer binding".into());
            }
        }
    }
    let mut buffers = Vec::new();
    for index in 0..desc.ConstantBuffers {
        let buffer =
            unsafe { shader.GetConstantBufferByIndex(index) }.ok_or("missing reflected buffer")?;
        let mut b = D3D12_SHADER_BUFFER_DESC::default();
        unsafe { buffer.GetDesc(&mut b) }.map_err(|e| e.to_string())?;
        if b.Type != D3D_CT_CBUFFER {
            continue;
        }
        if b.Size > 65536 || b.Variables > 64 {
            return Err("constant buffer exceeds size/variable bound".into());
        }
        let buffer_name = name(b.Name)?;
        let &(register, space) = bindings
            .get(&buffer_name)
            .ok_or("constant buffer binding missing")?;
        let mut variables = Vec::new();
        for index in 0..b.Variables {
            let var =
                unsafe { buffer.GetVariableByIndex(index) }.ok_or("missing reflected variable")?;
            let mut v = D3D12_SHADER_VARIABLE_DESC::default();
            unsafe { var.GetDesc(&mut v) }.map_err(|e| e.to_string())?;
            if v.StartOffset
                .checked_add(v.Size)
                .is_none_or(|end| end > b.Size)
            {
                return Err("reflected variable exceeds buffer".into());
            }
            let ty = unsafe { var.GetType() }.ok_or("reflected variable type missing")?;
            let mut t = D3D12_SHADER_TYPE_DESC::default();
            unsafe { ty.GetDesc(&mut t) }.map_err(|e| e.to_string())?;
            variables.push(json!({"name":name(v.Name)?,"offset":v.StartOffset,"size":v.Size,
                "flags":v.uFlags,"type":t.Type.0,"class":t.Class.0,"rows":t.Rows,"columns":t.Columns,"elements":t.Elements}));
        }
        buffers.push(json!({"name":buffer_name,"register":register,"space":space,"bytes":b.Size,"variables":variables}));
    }
    Ok(
        json!({"reflectionVerified":true,"reflectionBackend":backend,"shaderVersion":desc.Version,"buffers":buffers}),
    )
}
fn register_shader(bytes: &[u8]) -> [u8; 32] {
    let hash: [u8; 32] = Sha256::digest(bytes).into();
    if cache().lock().is_ok_and(|c| c.contains_key(&hash)) {
        return hash;
    }
    let mut layout = match reflect(bytes) {
        Ok(layout) => layout,
        Err(error) => match crate::native_shader_asset::metadata(hash) {
            Some(mut layout) => {
                layout["reflectionError"] = json!(error);
                layout
            }
            None => json!({"reflectionVerified":false,"error":error}),
        },
    };
    if layout.to_string().len() > MAX_LAYOUT_BYTES {
        layout = json!({"reflectionVerified":false,"error":"reflected layout exceeds 8 KiB"});
    }
    layout["bytecodeBytes"] = json!(bytes.len());
    if let Ok(mut c) = cache().lock() {
        if c.len() < 512 || c.contains_key(&hash) {
            c.insert(hash, layout);
        } else {
            OMITTED.fetch_add(1, Ordering::Relaxed);
        }
    }
    hash
}
fn bytecode(code: D3D12_SHADER_BYTECODE) -> Option<[u8; 32]> {
    if code.BytecodeLength == 0 {
        return Some([0; 32]);
    }
    if code.pShaderBytecode.is_null() || code.BytecodeLength > MAX_SHADER_BYTES {
        return None;
    }
    let mut bytes = vec![0; code.BytecodeLength];
    crate::world_hooks::read(code.pShaderBytecode as usize, &mut bytes).ok()?;
    Some(register_shader(&bytes))
}
pub(crate) unsafe fn tag_pipeline(
    raw: *mut c_void,
    vs: D3D12_SHADER_BYTECODE,
    ps: D3D12_SHADER_BYTECODE,
) {
    if !crate::native_clock::probe_enabled() {
        return;
    }
    let Some((v, p)) = bytecode(vs).zip(bytecode(ps)) else {
        return;
    };
    if v == [0; 32] && p == [0; 32] {
        return;
    }
    let Some(object) = (unsafe { IUnknown::from_raw_borrowed(&raw) }) else {
        return;
    };
    let Ok(pso) = object.cast::<ID3D12PipelineState>() else {
        return;
    };
    let mut bytes = [0u8; 64];
    bytes[..32].copy_from_slice(&v);
    bytes[32..].copy_from_slice(&p);
    if unsafe { pso.SetPrivateData(&SHADERS, 64, Some(bytes.as_ptr() as _)) }.is_ok() {
        PIPELINES.fetch_add(1, Ordering::Relaxed);
    }
}
pub(crate) unsafe fn pipeline(raw: *mut c_void) -> Option<ShaderPair> {
    if !crate::native_clock::probe_enabled() {
        return None;
    }
    let pso = unsafe { ID3D12PipelineState::from_raw_borrowed(&raw) }?;
    let mut size = 64;
    let mut bytes = [0u8; 64];
    unsafe { pso.GetPrivateData(&SHADERS, &mut size, Some(bytes.as_mut_ptr() as _)) }.ok()?;
    (size == 64).then(|| {
        (
            bytes[..32].try_into().unwrap(),
            bytes[32..].try_into().unwrap(),
        )
    })
}
pub(crate) unsafe fn tag_stream(raw: *mut c_void, bytes: &[u8]) {
    if !crate::native_clock::probe_enabled() {
        return;
    }
    let Some(objects) = crate::depth_pipeline::subobjects(bytes) else {
        return;
    };
    let mut vs = D3D12_SHADER_BYTECODE::default();
    let mut ps = D3D12_SHADER_BYTECODE::default();
    for (kind, bytes) in objects {
        let code = match kind {
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VS => &mut vs,
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PS => &mut ps,
            _ => continue,
        };
        code.pShaderBytecode = usize::from_le_bytes(bytes[..8].try_into().unwrap()) as _;
        code.BytecodeLength = usize::from_le_bytes(bytes[8..16].try_into().unwrap());
    }
    unsafe { tag_pipeline(raw, vs, ps) };
}
unsafe fn root_layout(blob: *const c_void, len: usize) -> Option<RootBinding> {
    if blob.is_null() || len == 0 || len > 65536 {
        return None;
    }
    let mut raw = std::ptr::null_mut();
    unsafe {
        D3D12CreateVersionedRootSignatureDeserializer(
            blob,
            len,
            &ID3D12VersionedRootSignatureDeserializer::IID,
            &mut raw,
        )
    }
    .map_err(|e| root_profile(json!({"stage":"deserialize","error":e.to_string()})))
    .ok()?;
    if raw.is_null() {
        return None;
    }
    let owner = unsafe { ID3D12VersionedRootSignatureDeserializer::from_raw(raw) };
    let versioned = unsafe { owner.GetRootSignatureDescAtVersion(D3D_ROOT_SIGNATURE_VERSION_1_0) }
        .map_err(|e| root_profile(json!({"stage":"convert","error":e.to_string()})))
        .ok()?;
    let desc = unsafe { versioned.as_ref()? };
    if desc.Version != D3D_ROOT_SIGNATURE_VERSION_1_0 {
        return None;
    }
    let desc = unsafe { desc.Anonymous.Desc_1_0 };
    if !(4..=64).contains(&desc.NumParameters) || desc.pParameters.is_null() {
        return None;
    }
    let parameters =
        unsafe { std::slice::from_raw_parts(desc.pParameters, desc.NumParameters as usize) };
    root_profile(
        json!({"stage":"parsed","parameters":parameters.iter().take(8).map(|p|{
        let mut v=json!({"type":p.ParameterType.0,"visibility":p.ShaderVisibility.0});
        if matches!(p.ParameterType,D3D12_ROOT_PARAMETER_TYPE_CBV|D3D12_ROOT_PARAMETER_TYPE_SRV|D3D12_ROOT_PARAMETER_TYPE_UAV) {
            let d=unsafe { p.Anonymous.Descriptor };v["register"]=json!(d.ShaderRegister);v["space"]=json!(d.RegisterSpace);
        }
        v
    }).collect::<Vec<_>>() }),
    );
    let v = &parameters[2];
    let p = &parameters[3];
    if v.ParameterType != D3D12_ROOT_PARAMETER_TYPE_CBV
        || p.ParameterType != D3D12_ROOT_PARAMETER_TYPE_CBV
        || !matches!(
            v.ShaderVisibility,
            D3D12_SHADER_VISIBILITY_VERTEX | D3D12_SHADER_VISIBILITY_ALL
        )
        || !matches!(
            p.ShaderVisibility,
            D3D12_SHADER_VISIBILITY_PIXEL | D3D12_SHADER_VISIBILITY_ALL
        )
    {
        return None;
    }
    let v = unsafe { v.Anonymous.Descriptor };
    let p = unsafe { p.Anonymous.Descriptor };
    Some([
        v.ShaderRegister,
        v.RegisterSpace,
        p.ShaderRegister,
        p.RegisterSpace,
    ])
}
pub(crate) unsafe fn tag_root(raw: *mut c_void, blob: *const c_void, len: usize) {
    if !crate::native_clock::probe_enabled() {
        return;
    }
    ROOTS.fetch_add(1, Ordering::Relaxed);
    let Some(binding) = (unsafe { root_layout(blob, len) }) else {
        return;
    };
    let Some(object) = (unsafe { IUnknown::from_raw_borrowed(&raw) }) else {
        return;
    };
    let Ok(root) = object.cast::<ID3D12RootSignature>() else {
        return;
    };
    let bytes: Vec<_> = binding.into_iter().flat_map(u32::to_le_bytes).collect();
    if unsafe { root.SetPrivateData(&ROOT, 16, Some(bytes.as_ptr() as _)) }.is_ok() {
        KNOWN_ROOTS.fetch_add(1, Ordering::Relaxed);
    }
}
pub(crate) unsafe fn root(raw: *mut c_void) -> Option<RootBinding> {
    if !crate::native_clock::probe_enabled() {
        return None;
    }
    let root = unsafe { ID3D12RootSignature::from_raw_borrowed(&raw) }?;
    let mut bytes = [0u8; 16];
    let mut size = 16;
    unsafe { root.GetPrivateData(&ROOT, &mut size, Some(bytes.as_mut_ptr() as _)) }.ok()?;
    if size != 16 {
        return None;
    }
    Some(std::array::from_fn(|i| {
        u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
    }))
}
pub(crate) fn key(hash: [u8; 32]) -> Option<String> {
    (hash != [0; 32]).then(|| hash.iter().map(|b| format!("{b:02x}")).collect())
}
pub(crate) fn metadata(hash: [u8; 32]) -> Option<Value> {
    cache().try_lock().ok()?.get(&hash).cloned()
}
pub(crate) fn status() -> Value {
    json!({"pipelines":PIPELINES.load(Ordering::Relaxed),"rootSignatures":ROOTS.load(Ordering::Relaxed),
        "mappedRootSignatures":KNOWN_ROOTS.load(Ordering::Relaxed),"omittedLayouts":OMITTED.load(Ordering::Relaxed),
        "shaderLayouts":cache().try_lock().ok().map(|c|c.len()),
        "assetUniformMetadata":crate::native_shader_asset::status(),
        "rootProfiles":ROOT_PROFILES.try_lock().ok().map(|v|v.clone()),
        "observerOnly":true,"gpuExecutionVerified":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    use hudhook::windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    use hudhook::windows::Win32::Graphics::Direct3D::{ID3DBlob, ID3DInclude};
    use hudhook::windows::core::s;
    #[test]
    fn reflection_uses_real_shader_register_offsets_types_and_rejects_invalid_bytecode()
    -> Result<(), Box<dyn std::error::Error>> {
        let source=b"cbuffer Scene:register(b0){float4x4 VIEW;float4 FOG_COLOR;float TIME;};float4 main(float4 p:POSITION):SV_POSITION{return mul(VIEW,p)+FOG_COLOR*TIME;}";
        let mut shader: Option<ID3DBlob> = None;
        unsafe {
            D3DCompile(
                source.as_ptr() as _,
                source.len(),
                None,
                None,
                None::<&ID3DInclude>,
                s!("main"),
                s!("vs_5_0"),
                0,
                0,
                &mut shader,
                None,
            )
        }?;
        let shader = shader.ok_or("test shader missing")?;
        let bytes = unsafe {
            std::slice::from_raw_parts(
                shader.GetBufferPointer() as *const u8,
                shader.GetBufferSize(),
            )
        };
        let value = reflect(bytes)?;
        let buffer = &value["buffers"][0];
        assert_eq!(buffer["register"], 0);
        assert_eq!(buffer["space"], 0);
        assert_eq!(buffer["bytes"], 96);
        let variables = buffer["variables"].as_array().unwrap();
        assert!(
            variables
                .iter()
                .any(|v| v["name"] == "FOG_COLOR" && v["offset"] == 64 && v["size"] == 16)
        );
        assert!(
            variables
                .iter()
                .any(|v| v["name"] == "TIME" && v["offset"] == 80 && v["size"] == 4)
        );
        assert!(reflect(b"not compiled shader code").is_err());
        let first = register_shader(bytes);
        assert_eq!(register_shader(bytes), first);
        assert_eq!(metadata(first).unwrap()["buffers"], value["buffers"]);
        Ok(())
    }
    #[test]
    fn root_layout_requires_the_actual_stage_visibility_and_register_mapping()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut parameters = [D3D12_ROOT_PARAMETER::default(); 4];
        for (index, p) in parameters.iter_mut().enumerate() {
            p.ParameterType = D3D12_ROOT_PARAMETER_TYPE_CBV;
            p.ShaderVisibility = if index.is_multiple_of(2) {
                D3D12_SHADER_VISIBILITY_VERTEX
            } else {
                D3D12_SHADER_VISIBILITY_PIXEL
            };
            p.Anonymous.Descriptor = D3D12_ROOT_DESCRIPTOR {
                ShaderRegister: if index < 2 { 1 } else { 0 },
                RegisterSpace: 0,
            };
        }
        let mut desc = D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: 4,
            pParameters: parameters.as_ptr(),
            ..Default::default()
        };
        let encode =
            |desc: &D3D12_ROOT_SIGNATURE_DESC| -> Result<ID3DBlob, Box<dyn std::error::Error>> {
                let mut bytes = None;
                unsafe {
                    D3D12SerializeRootSignature(
                        desc,
                        D3D_ROOT_SIGNATURE_VERSION_1_0,
                        &mut bytes,
                        None,
                    )
                }?;
                Ok(bytes.ok_or("root signature missing")?)
            };
        let bytes = encode(&desc)?;
        assert_eq!(
            unsafe { root_layout(bytes.GetBufferPointer(), bytes.GetBufferSize()) },
            Some([0; 4])
        );
        parameters[3].ShaderVisibility = D3D12_SHADER_VISIBILITY_ALL;
        parameters[3].Anonymous.Descriptor = D3D12_ROOT_DESCRIPTOR {
            ShaderRegister: 2,
            RegisterSpace: 0,
        };
        parameters[1].Anonymous.Descriptor = D3D12_ROOT_DESCRIPTOR {
            ShaderRegister: 5,
            RegisterSpace: 0,
        };
        desc.pParameters = parameters.as_ptr();
        let bytes = encode(&desc)?;
        assert_eq!(
            unsafe { root_layout(bytes.GetBufferPointer(), bytes.GetBufferSize()) },
            Some([0, 0, 2, 0])
        );
        parameters[3].ShaderVisibility = D3D12_SHADER_VISIBILITY_VERTEX;
        desc.pParameters = parameters.as_ptr();
        let bytes = encode(&desc)?;
        assert_eq!(
            unsafe { root_layout(bytes.GetBufferPointer(), bytes.GetBufferSize()) },
            None
        );
        desc.NumParameters = 3;
        let bytes = encode(&desc)?;
        assert_eq!(
            unsafe { root_layout(bytes.GetBufferPointer(), bytes.GetBufferSize()) },
            None
        );
        Ok(())
    }
    #[test]
    #[ignore = "requires a captured installed-game shader and the local Windows SDK DXC"]
    fn installed_dxil_shader_uses_the_same_public_reflection_reader()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::var_os("ARCTIC_SHADER_INPUT_FILE")
            .ok_or("ARCTIC_SHADER_INPUT_FILE missing")?;
        let bytes = std::fs::read(path)?;
        let layout = reflect(&bytes)?;
        assert_eq!(layout["reflectionBackend"], "dxc");
        assert_eq!(layout["buffers"][0]["register"], 1);
        assert_eq!(layout["buffers"][0]["bytes"], 160);
        assert!(
            layout["buffers"][0]["variables"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["name"] == "FogColor" && v["offset"] == 128)
        );
        println!("{}", layout);
        Ok(())
    }
    #[test]
    #[ignore = "exports reflection only for hash-matched local installed-game shader artifacts"]
    fn export_installed_shader_layouts() -> Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::var_os("ARCTIC_SHADER_INPUT_DIRECTORY").ok_or("shader directory missing")?;
        let mut count = 0;
        for file in std::fs::read_dir(root)?.take(128) {
            let file = file?.path();
            if file.extension().is_none_or(|x| x != "bin") {
                continue;
            }
            let bytes = std::fs::read(&file)?;
            if bytes.len() > MAX_SHADER_BYTES {
                return Err("shader artifact exceeds bound".into());
            }
            let hash = format!("{:x}", Sha256::digest(&bytes));
            if file.file_stem().is_none_or(|x| x != hash.as_str()) {
                return Err("shader artifact hash differs from file name".into());
            }
            let layout = reflect(&bytes)?;
            std::fs::write(
                file.with_extension("layout.json"),
                serde_json::to_vec_pretty(&layout)?,
            )?;
            count += 1;
        }
        assert!(count > 0);
        println!("Reflected {count} hash-matched installed-game shaders (declarations only)");
        Ok(())
    }
}
