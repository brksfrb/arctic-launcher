//! Guarded, owned RGBA copies from SerializedSkinImpl. No texture/image objects
//! are constructed in Minecraft and no native skin renderer is suppressed.
use crate::native_hash::{Sha256, hex};
use crate::world_hooks::read;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Serialize)]
pub struct Appearance {
    pub skin: String,
    #[serde(skip)]
    pub(crate) geometry: String,
    pub cape: Option<String>,
    pub slim: bool,
    #[serde(rename = "capeAbsent")]
    pub cape_absent: bool,
}
struct Cached {
    source: usize,
    at: Instant,
    appearance: Appearance,
}
static CACHE: OnceLock<Mutex<HashMap<String, Cached>>> = OnceLock::new();
static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);
static DESCRIPTION: Mutex<Option<serde_json::Value>> = Mutex::new(None);
pub(crate) fn status() -> serde_json::Value {
    serde_json::json!({"attempts":ATTEMPTS.load(Ordering::Relaxed),"copies":COPIES.load(Ordering::Relaxed),"source":DESCRIPTION.try_lock().ok().and_then(|v|v.clone())})
}
fn describe(source: usize) -> Option<serde_json::Value> {
    let geometry = crate::players::string(source.checked_add(0x80)?, 128);
    let geometry = match geometry.as_deref() {
        Some("geometry.humanoid.custom" | "geometry.humanoid.customSlim" | "geometry.humanoid") => {
            "standard"
        }
        Some(_) => "other",
        None => "empty-or-unreadable",
    };
    let mut flags = [0u8; 1];
    read(source.checked_add(0x20a)?, &mut flags).ok()?;
    let persona = (flags[0] <= 1).then_some(flags[0] != 0);
    read(source.checked_add(0x1b0)?, &mut flags).ok()?;
    let arm = (flags[0] <= 3).then_some(flags[0]);
    let mut header = [0u8; 48];
    read(source.checked_add(0xa0)?, &mut header).ok()?;
    let word = |at| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
    let size = u64::from_le_bytes(header[40..48].try_into().unwrap());
    let mut cape = [0u8; 48];
    read(source.checked_add(0xd0)?, &mut cape).ok()?;
    let mut cape_id = [0u8; 32];
    read(source.checked_add(0x178)?, &mut cape_id).ok()?;
    let mut cape_flags = [0u8; 2];
    read(source.checked_add(0x20a)?, &mut cape_flags).ok()?;
    let cape_word = |at| u32::from_le_bytes(cape[at..at + 4].try_into().unwrap());
    Some(
        serde_json::json!({"geometry":geometry,"persona":persona,"arm":arm,
        "format":(word(0)<=5).then(||word(0)),"width":(word(4)<=16384).then(||word(4)),"height":(word(8)<=16384).then(||word(8)),"depth":(word(12)<=16).then(||word(12)),"bytes":(size<=64*1024*1024).then_some(size),"capeAbsent":cape_absent(source),
        "capeImage":{"format":cape_word(0),"width":cape_word(4),"height":cape_word(8),"depth":cape_word(12),"bufferNull":cape[32..40]==[0;8],"bytes":u64::from_le_bytes(cape[40..48].try_into().unwrap())},
        "capeIdLength":u64::from_le_bytes(cape_id[16..24].try_into().unwrap()),"capeIdCapacity":u64::from_le_bytes(cape_id[24..32].try_into().unwrap()),"capeIdTerminator":cape_id[0]==0,"capeFlags":cape_flags}),
    )
}

// A failed image read is not evidence of absence. Require the classic skin's
// stable empty image, canonical empty cape ID and both persona flags clear.
// SerializedSkinImpl layout is checked against the pinned client's skin reader.
fn cape_absent(source: usize) -> bool {
    fn read_state(source: usize) -> Option<([u8; 48], [u8; 32], [u8; 2])> {
        let mut image = [0; 48];
        let mut id = [0; 32];
        let mut flags = [0; 2];
        read(source.checked_add(0xd0)?, &mut image).ok()?;
        read(source.checked_add(0x178)?, &mut id).ok()?;
        read(source.checked_add(0x20a)?, &mut flags).ok()?;
        Some((image, id, flags))
    }
    let Some(first) = read_state(source) else {
        return false;
    };
    let (image, id, flags) = first;
    let word = |at| u32::from_le_bytes(image[at..at + 4].try_into().unwrap());
    let empty = word(0) <= 5
        && word(4) == 0
        && word(8) == 0
        && word(12) <= 1
        // The installed image retains an allocated empty blob. Its byte count,
        // dimensions and cape ID prove absence; a non-null zero-length buffer
        // cannot provide any native cape pixels.
        && image[40..48] == [0; 8]
        && id[0] == 0
        && id[16..24] == [0; 8]
        && u64::from_le_bytes(id[24..32].try_into().unwrap()) == 15
        && flags == [0; 2];
    empty && read_state(source).is_some_and(|after| after == first)
}

fn image(address: usize, cape: bool) -> Option<crate::engine::Texture> {
    let mut header = [0u8; 48];
    read(address, &mut header).ok()?;
    let u32_at = |i| u32::from_le_bytes(header[i..i + 4].try_into().expect("four bytes"));
    let width = u32_at(4);
    let height = u32_at(8);
    if u32_at(0) != 4
        // The installed client's two-dimensional SkinImage uses depth zero.
        // Some callers explicitly use one; neither representation is a volume.
        || u32_at(12) > 1
        || !(64..=1024).contains(&width)
        || !width.is_multiple_of(64)
        || if cape {
            height.checked_mul(2)? != width
        } else {
            height != width && height.checked_mul(2)? != width
        }
    {
        return None;
    }
    let expected = width as usize * height as usize * 4;
    let source = u64::from_le_bytes(header[32..40].try_into().ok()?) as usize;
    let size = u64::from_le_bytes(header[40..48].try_into().ok()?) as usize;
    if size != expected || source < 0x10000 {
        return None;
    }
    let mut rgba = vec![0u8; expected];
    read(source, &mut rgba).ok()?;
    let mut after = [0u8; 48];
    read(address, &mut after).ok()?;
    if after != header {
        return None;
    }
    let mut hash = Sha256::new().ok()?;
    hash.update(&width.to_le_bytes()).ok()?;
    hash.update(&height.to_le_bytes()).ok()?;
    hash.update(&rgba).ok()?;
    let key = format!(
        "engine:{}/{}",
        if cape { "cape" } else { "skin" },
        hex(&hash.finish().ok()?)
    );
    Some(crate::engine::Texture {
        key,
        width,
        height,
        rgba,
    })
}
pub fn sample(id: &str, source: usize) -> Option<Appearance> {
    ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    if source == 0 {
        return None;
    }
    let cache = CACHE.get_or_init(Mutex::default);
    if let Ok(c) = cache.try_lock()
        && let Some(entry) = c
            .get(id)
            .filter(|e| e.source == source && e.at.elapsed() < Duration::from_millis(250))
    {
        let mut appearance = entry.appearance.clone();
        // A cached texture does not authorize a changed rig or persona. The
        // selected-geometry reference consumes this name within the same frame.
        let geometry = crate::players::string(source.checked_add(0x80)?, 128)?;
        let mut persona = [0];
        let mut arm = [0];
        read(source.checked_add(0x20a)?, &mut persona).ok()?;
        read(source.checked_add(0x1b0)?, &mut arm).ok()?;
        let expected_arm = if appearance.slim { 0 } else { 1 };
        if geometry != appearance.geometry || persona[0] != 0 || arm[0] != expected_arm {
            return None;
        }
        // Never carry a cached absence across an appearance change.
        appearance.cape_absent = cape_absent(source);
        return Some(appearance);
    }
    if let Ok(mut description) = DESCRIPTION.try_lock() {
        *description = describe(source);
    }
    // Classic UVs cannot represent persona or custom geometry. Keep those in
    // the native renderer instead of relabeling them as a standard Java body.
    let geometry = crate::players::string(source.checked_add(0x80)?, 128)?;
    if !matches!(
        geometry.as_str(),
        "geometry.humanoid.custom" | "geometry.humanoid.customSlim" | "geometry.humanoid"
    ) {
        return None;
    }
    let mut persona = [0];
    read(source.checked_add(0x20a)?, &mut persona).ok()?;
    if persona[0] != 0 {
        return None;
    }
    let mut arm = [0];
    read(source.checked_add(0x1b0)?, &mut arm).ok()?;
    let slim = match arm[0] {
        0 => true,
        1 => false,
        _ => return None,
    };
    let skin = image(source.checked_add(0xa0)?, false)?;
    let cape = image(source.checked_add(0xd0)?, true);
    let appearance = Appearance {
        skin: skin.key.clone(),
        geometry,
        cape: cape.as_ref().map(|t| t.key.clone()),
        slim,
        cape_absent: cape_absent(source),
    };
    // The renderer inbox owns copies; no engine pointer crosses into JNI.
    crate::engine::upload(skin).ok()?;
    if let Some(cape) = cape {
        crate::engine::upload(cape).ok()?;
    }
    COPIES.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut c) = cache.try_lock() {
        if c.len() >= 128
            && !c.contains_key(id)
            && let Some(key) = c.iter().min_by_key(|(_, e)| e.at).map(|(k, _)| k.clone())
        {
            c.remove(&key);
        }
        c.insert(
            id.into(),
            Cached {
                source,
                at: Instant::now(),
                appearance: appearance.clone(),
            },
        );
    }
    Some(appearance)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cape_absence_requires_a_readable_stable_empty_classic_appearance() {
        let mut source = [0u8; 0x210];
        source[0x178 + 24..0x178 + 32].copy_from_slice(&15u64.to_le_bytes());
        let check = |source: &[u8]| {
            std::hint::black_box(source);
            cape_absent(source.as_ptr() as usize)
        };
        assert!(check(&source));
        source[0xd0 + 32] = 1;
        assert!(check(&source));
        source[0xd0 + 32] = 0;
        source[0xd0 + 12] = 1;
        assert!(check(&source));
        source[0xd0 + 12] = 2;
        assert!(!check(&source));
        source[0xd0 + 12] = 0;
        for at in [
            0xd0 + 4,
            0xd0 + 8,
            0xd0 + 40,
            0x178,
            0x178 + 16,
            0x20a,
            0x20b,
        ] {
            source[at] = 1;
            assert!(!check(&source), "offset {at:x}");
            source[at] = 0;
        }
        source[0x178 + 24] = 14;
        assert!(!check(&source));
        assert!(!cape_absent(1));
    }
    #[test]
    fn image_copies_owned_rgba_and_rejects_wrong_formats_sizes_depth_and_dead_pointers() {
        let mut bytes = vec![0u8; 64 * 32 * 4];
        for p in bytes.as_chunks_mut::<4>().0 {
            p.copy_from_slice(&[12, 34, 56, 255]);
        }
        let mut image_header = [0u8; 48];
        for (at, value) in [(0, 4u32), (4, 64), (8, 32), (12, 1)] {
            image_header[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        image_header[32..40].copy_from_slice(&(bytes.as_ptr() as u64).to_le_bytes());
        image_header[40..48].copy_from_slice(&(bytes.len() as u64).to_le_bytes());
        let copy = image(image_header.as_ptr() as usize, false).unwrap();
        assert_eq!(copy.rgba[..4], [12, 34, 56, 255]);
        assert_eq!(
            image(image_header.as_ptr() as usize, true).unwrap().height,
            32
        );
        image_header[12..16].copy_from_slice(&0u32.to_le_bytes());
        std::hint::black_box(&image_header);
        assert_eq!(
            image(image_header.as_ptr() as usize, false).unwrap().key,
            copy.key
        );
        image_header[..4].copy_from_slice(&3u32.to_le_bytes());
        std::hint::black_box(&image_header);
        assert!(image(image_header.as_ptr() as usize, false).is_none());
        image_header[..4].copy_from_slice(&4u32.to_le_bytes());
        image_header[12..16].copy_from_slice(&2u32.to_le_bytes());
        std::hint::black_box(&image_header);
        assert!(image(image_header.as_ptr() as usize, false).is_none());
        image_header[12..16].copy_from_slice(&1u32.to_le_bytes());
        image_header[32..40].copy_from_slice(&1u64.to_le_bytes());
        std::hint::black_box(&image_header);
        assert!(image(image_header.as_ptr() as usize, false).is_none());
        assert_eq!(copy.rgba[..4], [12, 34, 56, 255]);
    }
}
