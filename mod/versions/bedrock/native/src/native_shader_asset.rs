//! Read-only metadata for stripped BGFX-v5 shaders in one pinned Actor asset.
//! Asset uniform labels are separate from D3D reflection/material ownership.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    sync::{Mutex, OnceLock},
};

const ASSET_HASH: &str = "8720b71a8e7b6c4d5e9c12b6a8293734919030559d1e3ca0ad452f532f3ae9e6";
const MAX_ASSET: usize = 32 * 1024 * 1024;
const MAX_SHADER: usize = 4 * 1024 * 1024;
const MAX_LAYOUT: usize = 8192;
pub(crate) const MAX_VECTOR_BYTES: usize = 512;
struct State {
    layouts: HashMap<[u8; 32], Option<Value>>,
    error: Option<String>,
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        Mutex::new(State {
            layouts: HashMap::new(),
            error: None,
        })
    })
}
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let value = self.data.get(self.at..end)?;
        self.at = end;
        Some(value)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}
fn word(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// Validate SM4/SM5.0 declaration tokens and its sole CB0, without interpreting
/// instructions, matrices, texture resources or executing/disassembling code.
fn cb0(data: &[u8], stage: u32) -> Option<(u32, Option<u32>)> {
    if data.len() < 32
        || data.get(..4)? != b"DXBC"
        || word(data, 20)? != 1
        || word(data, 24)? as usize != data.len()
    {
        return None;
    }
    let count = word(data, 28)? as usize;
    if !(1..=64).contains(&count) {
        return None;
    }
    let header = 32 + count * 4;
    let mut ranges = vec![(0, header)];
    let mut program = None;
    for index in 0..count {
        let at = word(data, 32 + index * 4)? as usize;
        let len = word(data, at.checked_add(4)?)? as usize;
        let end = at.checked_add(8)?.checked_add(len)?;
        if at < header || end > data.len() {
            return None;
        }
        ranges.push((at, end));
        let tag = data.get(at..at + 4)?;
        if tag == b"SHDR" || tag == b"SHEX" {
            if program.is_some() {
                return None;
            }
            program = Some(data.get(at + 8..end)?);
        }
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|p| p[0].1 > p[1].0) {
        return None;
    }
    let code = program?;
    let version = word(code, 0)?;
    if version >> 16 != stage
        || !matches!((version >> 4) & 15, 4 | 5)
        || version & 15 != 0
        || code.len() % 4 != 0
        || word(code, 4)? as usize != code.len() / 4
    {
        return None;
    }
    let mut at = 8;
    let mut buffer = None;
    while at < code.len() {
        let token = word(code, at)?;
        let opcode = token & 0x7ff;
        let length = if opcode == 53 {
            word(code, at + 4)? as usize
        } else {
            ((token >> 24) & 127) as usize
        };
        if length == 0 {
            return None;
        }
        let end = at.checked_add(length.checked_mul(4)?)?;
        if end > code.len() {
            return None;
        }
        if opcode == 89 {
            // DCL_CONSTANT_BUFFER, immediate CB register and vector count.
            let operand = word(code, at + 4)?;
            if length != 4
                || operand >> 31 != 0
                || ((operand >> 12) & 255) != 8
                || ((operand >> 20) & 3) != 2
                || ((operand >> 22) & 63) != 0
                || word(code, at + 8)? != 0
                || buffer.is_some()
            {
                return None;
            }
            let rows = word(code, at + 12)?;
            if !(1..=4096).contains(&rows) {
                return None;
            }
            buffer = Some(rows * 16);
        }
        at = end;
    }
    Some((version, buffer))
}

fn candidate(data: &[u8], start: usize) -> Option<([u8; 32], Value)> {
    let magic = data.get(start..start.checked_add(4)?)?;
    let stage = match magic {
        b"VSH\x05" => 1,
        b"FSH\x05" => 0,
        _ => return None,
    };
    let mut reader = Reader {
        data,
        at: start + 4,
    };
    reader.u32()?;
    let count = reader.u16()? as usize;
    if count > 128 {
        return None;
    }
    let mut uniforms = Vec::with_capacity(count);
    let mut names = HashSet::new();
    let mut ranges = Vec::new();
    for _ in 0..count {
        let len = reader.u8()? as usize;
        if !(1..=96).contains(&len) {
            return None;
        }
        let bytes = reader.take(len)?;
        if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_'
            || !bytes
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            return None;
        }
        let name = std::str::from_utf8(bytes).ok()?.to_owned();
        if !names.insert(name.clone()) {
            return None;
        }
        let flags = reader.u8()?;
        let kind = flags & 15;
        let array = reader.u8()? as u32;
        let offset = reader.u16()? as u32;
        let registers = reader.u16()? as u32;
        if flags & !0x3f != 0 || !matches!(kind, 0 | 2 | 3 | 4) {
            return None;
        }
        if kind != 0 {
            let per = match kind {
                2 => 1,
                3 => 3,
                4 => 4,
                _ => return None,
            };
            if registers != array.max(1) * per || !offset.is_multiple_of(16) {
                return None;
            }
            let end = offset.checked_add(registers.checked_mul(16)?)?;
            if end > 65536 {
                return None;
            }
            ranges.push((offset, end));
        }
        uniforms.push((name, kind, array, offset, registers));
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|p| p[0].1 > p[1].0) {
        return None;
    }
    let size = reader.u32()? as usize;
    if !(32..=MAX_SHADER).contains(&size) {
        return None;
    }
    let code = reader.take(size)?;
    let (version, buffer) = cb0(code, stage)?;
    let hash: [u8; 32] = Sha256::digest(code).into();
    if ranges
        .iter()
        .any(|range| buffer.is_none_or(|bytes| range.1 > bytes))
    {
        return None;
    }
    let mut variables = Vec::new();
    let mut omitted = 0;
    for (name, kind, array, offset, registers) in uniforms {
        if kind == 0 {
            continue;
        }
        // Vector storage is known. Matrix major order is not supplied by this
        // wrapper, so never invent a reflected matrix class or decode its data.
        if kind != 2 {
            omitted += 1;
            continue;
        }
        variables.push(
            json!({"name":name,"offset":offset,"size":registers*16,"used":true,
            "type":3,"class":1,"rows":1,"columns":4,"elements":array}),
        );
    }
    if variables.len() > 64 {
        return None;
    }
    let buffers=buffer.map(|size|vec![json!({"name":"BGFX_CB0","register":0,"space":0,"bytes":size,"variables":variables})]).unwrap_or_default();
    let layout = json!({"reflectionVerified":false,"uniformMetadataVerified":true,"layoutSource":"pinned-bgfx5-asset",
        "asset":"Actor.material.bin","assetSha256":ASSET_HASH,"shaderVersion":version,"shaderStage":stage,
        "buffers":buffers,"omittedMatrixUniforms":omitted,"runtimeMaterialVerified":false,"completeInputCoverageVerified":false});
    (layout.to_string().len() <= MAX_LAYOUT).then_some((hash, layout))
}
fn load() -> Result<HashMap<[u8; 32], Option<Value>>, String> {
    let image = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = image
        .parent()
        .ok_or("missing game content directory")?
        .join("data/renderer/materials/Actor.material.bin");
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    file.take(MAX_ASSET as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > MAX_ASSET || format!("{:x}", Sha256::digest(&data)) != ASSET_HASH {
        return Err("installed Actor material fingerprint differs".into());
    }
    let mut layouts = HashMap::new();
    for start in 0..data.len().saturating_sub(3) {
        if !matches!(&data[start..start + 4], b"VSH\x05" | b"FSH\x05") {
            continue;
        }
        let Some((hash, layout)) = candidate(&data, start) else {
            continue;
        };
        match layouts.entry(hash) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Some(layout));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if entry.get().as_ref() != Some(&layout) {
                    entry.insert(None);
                }
            }
        }
        if layouts.len() > 4096 {
            return Err("Actor shader index exceeds bound".into());
        }
    }
    if layouts.is_empty() {
        return Err("no supported BGFX-v5 Actor shader metadata".into());
    }
    Ok(layouts)
}
pub(crate) fn configure() {
    let next = match load() {
        Ok(layouts) => State {
            layouts,
            error: None,
        },
        Err(error) => State {
            layouts: HashMap::new(),
            error: Some(error),
        },
    };
    if let Ok(mut current) = state().lock() {
        *current = next;
    }
}
pub(crate) fn metadata(hash: [u8; 32]) -> Option<Value> {
    state().lock().ok()?.layouts.get(&hash)?.clone()
}
pub(crate) fn status() -> Value {
    let Ok(current) = state().lock() else {
        return Value::Null;
    };
    json!({"assetSha256":ASSET_HASH,"indexed":current.layouts.len(),"ambiguous":current.layouts.values().filter(|v|v.is_none()).count(),
        "error":current.error,"reflectionVerified":false,"completeInputCoverageVerified":false})
}

/// Exact shader-declared vector ranges, never guessed matrices or CB registers.
/// The GPU observer separately bounds the whole declared CB in one live mapping
/// before copying each range. The callback must return owned bytes only.
pub(crate) struct VectorLayout {
    pub(crate) buffer_bytes: usize,
    fields: Vec<(String, usize, usize)>,
    omitted_matrices: u64,
}
pub(crate) fn vectors(hash: [u8; 32], register: u32, space: u32) -> Option<VectorLayout> {
    if register != 0 || space != 0 {
        return None;
    }
    let layout = metadata(hash)?;
    let buffers = layout["buffers"].as_array()?;
    if buffers.len() != 1 {
        return None;
    }
    let buffer = &buffers[0];
    let buffer_bytes = usize::try_from(buffer["bytes"].as_u64()?).ok()?;
    if !(16..=65536).contains(&buffer_bytes) {
        return None;
    }
    let mut fields = Vec::new();
    for variable in buffer["variables"].as_array()? {
        let at = usize::try_from(variable["offset"].as_u64()?).ok()?;
        let size = usize::try_from(variable["size"].as_u64()?).ok()?;
        if size == 0 || size % 16 != 0 || at.checked_add(size)? > buffer_bytes {
            return None;
        }
        fields.push((variable["name"].as_str()?.to_owned(), at, size));
    }
    if fields.len() > 64 {
        return None;
    }
    Some(VectorLayout {
        buffer_bytes,
        fields,
        omitted_matrices: layout["omittedMatrixUniforms"].as_u64()?,
    })
}
impl VectorLayout {
    pub(crate) fn capture(
        &self,
        mut copy: impl FnMut(usize, &mut [u8]) -> Option<()>,
    ) -> Option<Value> {
        let mut values = Vec::new();
        let mut copied = 0;
        let mut omitted = 0;
        for (name, at, size) in &self.fields {
            if *size > MAX_VECTOR_BYTES - copied {
                omitted += 1;
                continue;
            }
            let mut bytes = vec![0u8; *size];
            copy(*at, &mut bytes)?;
            copied += *size;
            let bits: Vec<u32> = bytes
                .chunks_exact(4)
                .map(|part| u32::from_le_bytes(part.try_into().unwrap()))
                .collect();
            let floats: Vec<Option<f32>> = bits
                .iter()
                .map(|bits| {
                    let v = f32::from_bits(*bits);
                    v.is_finite().then_some(v)
                })
                .collect();
            values.push(json!({"name":name,"offset":at,"bytes":size,
                "sha256":format!("{:x}",Sha256::digest(&bytes)),"bits":bits,"values":floats}));
        }
        Some(
            json!({"assetSha256":ASSET_HASH,"uniformMetadataVerified":true,"values":values,
            "declaredBufferBytes":self.buffer_bytes,"copiedBytes":copied,"omittedVectorUniforms":omitted,
            "omittedMatrixUniforms":self.omitted_matrices,"boundedVectorsOnly":true,"prefixOnly":false,
            "coherentSnapshotVerified":false,"completeInputCoverageVerified":false,
            "runtimeMaterialVerified":false,"gpuExecutionVerified":false}),
        )
    }
}
