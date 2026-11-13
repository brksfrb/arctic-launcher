//! Owned target matrices from the shared Java rig at a checked native clock.
//! This module observes target math only; it does not patch native equipment.
use jni::{
    JavaVM,
    objects::{GlobalRef, JClass, JDoubleArray, JValue},
};
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

struct Bridge {
    vm: Arc<JavaVM>,
    class: GlobalRef,
}
static BRIDGE: OnceLock<Bridge> = OnceLock::new();
static CALLS: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);
static REJECTED: AtomicU64 = AtomicU64::new(0);
thread_local! {static ACTIVE:Cell<bool>=const {Cell::new(false)};}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(false));
    }
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Target {
    pub token: String,
    pub animation_frame: u64,
    pub time_ms: u64,
    pub sample_time_ms: f64,
    pub seconds: f32,
    pub reference_parts: [[f32; 12]; 6],
    pub parts: [[f32; 12]; 6],
}
pub(crate) fn bind(vm: Arc<JavaVM>, class: GlobalRef) -> Result<(), String> {
    BRIDGE
        .set(Bridge { vm, class })
        .map_err(|_| "native emote math bridge is already bound".into())
}
fn rigid(parts: &[[f32; 12]; 6]) -> bool {
    parts.iter().all(|m| {
        if m.iter().any(|v| !v.is_finite() || v.abs() > 256.) {
            return false;
        }
        for a in 0..3 {
            for b in 0..3 {
                let dot: f32 = (0..3).map(|r| m[r * 4 + a] * m[r * 4 + b]).sum();
                let expected = if a == b { 1. } else { 0. };
                if (dot - expected).abs() > 0.001 {
                    return false;
                }
            }
        }
        let det = m[0] * (m[5] * m[10] - m[6] * m[9]) - m[1] * (m[4] * m[10] - m[6] * m[8])
            + m[2] * (m[4] * m[9] - m[5] * m[8]);
        (det - 1.).abs() <= 0.001
    })
}
fn query(
    token: &str,
    session: &str,
    actor: &str,
    timing: &str,
    reference: &[[f32; 12]; 6],
) -> Option<[f64; 74]> {
    let bridge = BRIDGE.get()?;
    let mut env = bridge.vm.attach_current_thread_as_daemon().ok()?;
    let class: &JClass = bridge.class.as_obj().into();
    let result = env.with_local_frame(16, |env| -> jni::errors::Result<Option<[f64; 74]>> {
        let token = env.new_string(token)?;
        let session = env.new_string(session)?;
        let actor = env.new_string(actor)?;
        let timing = env.new_string(timing)?;
        let baseline = env.new_float_array(72)?;
        let flat: Vec<f32> = reference.iter().flatten().copied().collect();
        env.set_float_array_region(&baseline, 0, &flat)?;
        let output = env
            .call_static_method(
                class,
                "sampleEmotePose",
                "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;[F)[D",
                &[
                    JValue::Object(&token),
                    JValue::Object(&session),
                    JValue::Object(&actor),
                    JValue::Object(&timing),
                    JValue::Object(&baseline),
                ],
            )?
            .l()?;
        if output.is_null() {
            return Ok(None);
        }
        let output = JDoubleArray::from(output);
        if env.get_array_length(&output)? != 74 {
            return Ok(None);
        }
        let mut copied = [0.; 74];
        env.get_double_array_region(&output, 0, &mut copied)?;
        Ok(Some(copied))
    });
    match result {
        Ok(value) => value,
        Err(_) => {
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
            None
        }
    }
}
pub(crate) fn sample(
    token: &str,
    actor: &str,
    partial: f32,
    pose: &crate::native_body::SkinPose,
) -> Option<Target> {
    if !crate::native_body::enabled()
        || token.len() != 64
        || !token
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        || !rigid(&pose.parts)
    {
        return None;
    }
    if ACTIVE.with(|active| active.replace(true)) {
        return None;
    }
    let _guard = Guard;
    CALLS.fetch_add(1, Ordering::Relaxed);
    let result = (|| {
        let (session, timing) = crate::world_hooks::emote_timing(partial)?;
        let json = timing.to_string();
        if json.len() > 16 * 1024 {
            return None;
        }
        let values = query(token, &session, actor, &json, &pose.parts)?;
        if values.iter().any(|v| !v.is_finite())
            || values[0] < 0.
            || values[0] > 1_000_000_000_000.
            || values[1] < 0.
            || values[1] > 86400.
        {
            return None;
        }
        let seconds = values[1] as f32;
        if f64::from(seconds) != values[1] {
            return None;
        }
        let parts = std::array::from_fn(|i| std::array::from_fn(|j| values[2 + i * 12 + j] as f32));
        if !rigid(&parts) || (0..72).any(|i| f64::from(parts[i / 12][i % 12]) != values[2 + i]) {
            return None;
        }
        let (after_session, after) = crate::world_hooks::emote_timing(partial)?;
        if after_session != session || after != timing {
            return None;
        }
        Some(Target {
            token: token.into(),
            animation_frame: pose.animation_frame,
            time_ms: values[0] as u64,
            sample_time_ms: values[0],
            seconds,
            reference_parts: pose.parts,
            parts,
        })
    })();
    if result.is_some() {
        COPIES.fetch_add(1, Ordering::Relaxed);
    } else {
        REJECTED.fetch_add(1, Ordering::Relaxed);
    }
    result
}
pub(crate) fn status() -> serde_json::Value {
    serde_json::json!({"bridgeBound":BRIDGE.get().is_some(),"calls":CALLS.load(Ordering::Relaxed),"copies":COPIES.load(Ordering::Relaxed),
        "rejected":REJECTED.load(Ordering::Relaxed),"nativeEquipmentPoseApplied":false,"nativeEmotePoseVerified":false})
}
