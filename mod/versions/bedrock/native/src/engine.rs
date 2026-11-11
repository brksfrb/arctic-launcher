//! Versioned input boundary for a Bedrock engine adapter. This module does not
//! guess private game layouts or claim that an engine producer is installed.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;

const MAX_JSON: usize = 1024 * 1024;
const MAX_PIXELS: usize = 3840 * 2160;
const MAX_TEXTURE_BYTES: usize = 64 * 1024 * 1024;
const FRAME_LIFETIME: Duration = Duration::from_millis(500);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Header {
    schema: u32,
    width: usize,
    height: usize,
    sequence: u64,
    session: String,
    players: Vec<serde_json::Value>,
}

pub struct Frame {
    pub json: String,
    pub depth: Vec<f32>,
    pub session: String,
    pub sequence: u64,
    at: Instant,
}
pub struct Texture {
    pub key: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
#[derive(Default)]
struct Inbox {
    frame: Option<Arc<Frame>>,
    textures: VecDeque<Texture>,
    texture_bytes: usize,
}
static INBOX: OnceLock<Mutex<Inbox>> = OnceLock::new();
static SURFACE: AtomicU64 = AtomicU64::new(0);
fn inbox() -> &'static Mutex<Inbox> {
    INBOX.get_or_init(Mutex::default)
}

fn checked_frame(json: &str, depth: &[f32]) -> Result<Header, ()> {
    if json.len() > MAX_JSON {
        return Err(());
    }
    let h: Header = serde_json::from_str(json).map_err(|_| ())?;
    if h.schema != 1
        || h.sequence > i64::MAX as u64
        || !(1..=3840).contains(&h.width)
        || !(1..=2160).contains(&h.height)
        || h.width * h.height != depth.len()
        || h.session.is_empty()
        || h.session.len() > 64
        || !h
            .session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        || h.players.len() > 128
        || depth
            .iter()
            .any(|d| !d.is_finite() || !(0.0..=1.0).contains(d))
    {
        return Err(());
    }
    Ok(h)
}

impl Inbox {
    fn latest(&self) -> Option<Arc<Frame>> {
        self.frame
            .as_ref()
            .filter(|f| f.at.elapsed() < FRAME_LIFETIME)
            .cloned()
    }
    fn submit(&mut self, json: &str, depth: &[f32]) -> Result<(), i32> {
        let h = checked_frame(json, depth).map_err(|_| -1)?;
        if self
            .frame
            .as_ref()
            .is_some_and(|f| f.session == h.session && f.sequence >= h.sequence)
        {
            return Err(-2);
        }
        self.frame = Some(Arc::new(Frame {
            json: json.into(),
            depth: depth.to_vec(),
            session: h.session,
            sequence: h.sequence,
            at: Instant::now(),
        }));
        Ok(())
    }
    fn texture(&mut self, key: &str, width: u32, height: u32, rgba: &[u8]) -> Result<(), i32> {
        if !key.starts_with("engine:")
            || key.len() > 127
            || key.len() <= 7
            || !key[7..]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'/' | b'-'))
            || !(1..=1024).contains(&width)
            || !(1..=1024).contains(&height)
            || rgba.len() != width as usize * height as usize * 4
        {
            return Err(-1);
        }
        let replace = self.textures.iter().position(|t| t.key == key);
        let old_bytes = replace.map_or(0, |i| self.textures[i].rgba.len());
        if self.texture_bytes - old_bytes + rgba.len() > MAX_TEXTURE_BYTES
            || replace.is_none() && self.textures.len() >= 128
        {
            return Err(-3);
        }
        if let Some(i) = replace
            && let Some(old) = self.textures.remove(i)
        {
            self.texture_bytes -= old.rgba.len();
        }
        self.texture_bytes += rgba.len();
        self.textures.push_back(Texture {
            key: key.into(),
            width,
            height,
            rgba: rgba.to_vec(),
        });
        Ok(())
    }
}

pub fn surface(width: i32, height: i32) {
    SURFACE.store((width as u64) << 32 | height as u64, Ordering::Release);
}
pub fn latest() -> Option<Arc<Frame>> {
    inbox().lock().ok()?.latest()
}
pub(crate) fn submit(json: &str, depth: &[f32]) -> Result<(), i32> {
    inbox().lock().map_err(|_| -4)?.submit(json, depth)
}
pub(crate) fn upload(texture: Texture) -> Result<(), i32> {
    inbox().lock().map_err(|_| -4)?.texture(
        &texture.key,
        texture.width,
        texture.height,
        &texture.rgba,
    )
}
pub fn textures() -> Vec<Texture> {
    if let Ok(mut state) = inbox().lock() {
        state.texture_bytes = 0;
        state.textures.drain(..).collect()
    } else {
        Vec::new()
    }
}

/// Returns width in the upper 32 bits and height in the lower 32 bits; 0 before
/// the first render. This is the world-overlay surface, fitted to 3840x2160;
/// it is independent of the Java HUD's logical dimensions.
#[unsafe(no_mangle)]
pub extern "C" fn arctic_bedrock_surface_v1() -> u64 {
    SURFACE.load(Ordering::Acquire)
}

/// Upload one owned world frame. Returns 0 on success, -1 for invalid data,
/// -2 for an old/duplicate sequence, -3 for a full queue, -4 on internal failure.
///
/// # Safety
/// Both non-null pointers must address the declared number of readable elements
/// for the duration of this call. Depth must be f32-aligned. Buffers are copied;
/// the engine can release/reuse them immediately after return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn arctic_bedrock_world_v1(
    json: *const u8,
    len: usize,
    depth: *const f32,
    pixels: usize,
) -> i32 {
    if json.is_null()
        || depth.is_null()
        || !depth.is_aligned()
        || len == 0
        || len > MAX_JSON
        || pixels == 0
        || pixels > MAX_PIXELS
    {
        return -1;
    }
    std::panic::catch_unwind(|| {
        // SAFETY: checked sizes/null/alignment above; readable storage is the
        // explicit contract of this in-process engine API.
        let (json, depth) = unsafe {
            (
                std::slice::from_raw_parts(json, len),
                std::slice::from_raw_parts(depth, pixels),
            )
        };
        let Ok(json) = std::str::from_utf8(json) else {
            return -1;
        };
        inbox()
            .lock()
            .map_or(-4, |mut s| s.submit(json, depth).err().unwrap_or(0))
    })
    .unwrap_or(-4)
}

/// Upload an RGBA texture for use by a world player. Same return codes as world.
///
/// # Safety
/// key and rgba must point to key_len bytes and bytes bytes of readable storage,
/// respectively, for this call. Both are copied before this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn arctic_bedrock_texture_v1(
    key: *const u8,
    key_len: usize,
    width: u32,
    height: u32,
    rgba: *const u8,
    bytes: usize,
) -> i32 {
    if key.is_null() || rgba.is_null() || key_len == 0 || key_len > 127 || bytes > 4 * 1024 * 1024 {
        return -1;
    }
    std::panic::catch_unwind(|| {
        // SAFETY: the documented input buffers remain readable during this call.
        let (key, data) = unsafe {
            (
                std::slice::from_raw_parts(key, key_len),
                std::slice::from_raw_parts(rgba, bytes),
            )
        };
        let Ok(key) = std::str::from_utf8(key) else {
            return -1;
        };
        inbox().lock().map_or(-4, |mut s| {
            s.texture(key, width, height, data).err().unwrap_or(0)
        })
    })
    .unwrap_or(-4)
}

/// Drop the current world on disconnect, before another connection can draw it.
#[unsafe(no_mangle)]
pub extern "C" fn arctic_bedrock_clear_world_v1() {
    if let Ok(mut state) = inbox().lock() {
        state.frame = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(sequence: u64) -> String {
        serde_json::json!({"schema":1,"session":"offline-fixture","sequence":sequence,"width":2,"height":2,"players":[]}).to_string()
    }
    #[test]
    fn copies_inputs_and_rejects_old_frames_and_invalid_depth() {
        let mut state = Inbox::default();
        let mut depth = vec![1.0; 4];
        state.submit(&frame(1), &depth).unwrap();
        depth[0] = 0.1;
        assert_eq!(state.frame.as_ref().unwrap().depth[0], 1.0);
        assert_eq!(state.submit(&frame(1), &depth), Err(-2));
        state.submit(&frame(2), &depth).unwrap();
        depth[0] = f32::NAN;
        assert_eq!(state.submit(&frame(3), &depth), Err(-1));
        assert_eq!(state.frame.as_ref().unwrap().sequence, 2);
        assert!(checked_frame(&frame(1), &[0.5]).is_err());
    }
    #[test]
    fn replaces_pending_textures_without_growing_the_queue() {
        let mut state = Inbox::default();
        let mut rgba = vec![255; 16];
        state.texture("engine:skin", 2, 2, &rgba).unwrap();
        rgba[0] = 99;
        assert_eq!(state.textures[0].rgba[0], 255);
        state.texture("engine:skin", 2, 2, &rgba).unwrap();
        assert_eq!(state.textures.len(), 1);
        assert_eq!(state.texture_bytes, 16);
        assert_eq!(state.texture("look:arbitrary", 2, 2, &rgba), Err(-1));
        assert_eq!(state.texture("engine:skin", 1025, 2, &rgba), Err(-1));
    }
    #[test]
    fn stale_worlds_expire_without_resetting_the_session_sequence() {
        let mut state = Inbox::default();
        state.submit(&frame(1), &[1.0; 4]).unwrap();
        assert!(state.latest().is_some());
        Arc::get_mut(state.frame.as_mut().unwrap()).unwrap().at =
            Instant::now() - FRAME_LIFETIME - Duration::from_millis(1);
        assert!(state.latest().is_none());
        assert_eq!(state.submit(&frame(1), &[1.0; 4]), Err(-2));
        state.submit(&frame(2), &[1.0; 4]).unwrap();
        assert!(state.latest().is_some());
        assert_eq!(state.submit(&frame(u64::MAX), &[1.0; 4]), Err(-1));
    }
}
