//! Owned wing observations from an actual selected attachable draw. A separate
//! typed receipt and explicit normal/probe controls govern native suppression.
use crate::animation_pose::Pose;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static PROBE_ENABLED: AtomicBool = AtomicBool::new(false);
static SUPPRESSED: AtomicU64 = AtomicU64::new(0);
pub(crate) fn configure_probe(enabled: bool) {
    PROBE_ENABLED.store(enabled, Ordering::Release);
}
pub(crate) fn enabled() -> bool {
    crate::native_body::enabled()
        && (crate::native_body::user_enabled() || PROBE_ENABLED.load(Ordering::Acquire))
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Replacement {
    pub cape: String,
    pub texture: String,
    pub geometry: String,
    pub animation_frame: u64,
    pub mesh_draws: u32,
    pub wings: [[f32; 12]; 2],
}
pub(crate) fn record_suppressed(replacement: Replacement) -> bool {
    if !enabled() || !crate::native_transform::record_elytra(replacement) {
        return false;
    }
    SUPPRESSED.fetch_add(1, Ordering::Relaxed);
    true
}
pub(crate) fn status() -> serde_json::Value {
    serde_json::json!({"replacementEnabled":enabled(),"suppressed":SUPPRESSED.load(Ordering::Relaxed),
        "nativeElytraReplacementVerified":false,"deterministicWingExportVerified":false})
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WingPose {
    pub geometry: String,
    pub hierarchy: u32,
    pub animation_frame: u64,
    /// Left, right; row-major bone-to-model 3x4 matrices, translations in blocks.
    pub wings: [[f32; 12]; 2],
}

/// The vanilla geometry has exactly body/left_wing/right_wing, all with a
/// (0,24,0) rest pivot. Completed matrices already include controller blends,
/// posture and the nonuniform wing scale. Do not traverse unresolved parents
/// or synthesize poses from an entity's gliding bit.
pub(crate) fn observe(pose: &Pose, selected: u32, geometry: &str) -> Option<WingPose> {
    if geometry != "geometry.elytra" {
        return None;
    }
    let mut candidates = pose.hierarchies.iter().filter(|h| h.index == selected);
    let hierarchy = candidates.next()?;
    if candidates.next().is_some() || hierarchy.bones.len() != 3 {
        return None;
    }
    let mut wings = [[0.; 12]; 2];
    for (i, name) in ["body", "left_wing", "right_wing"].iter().enumerate() {
        let mut candidates = hierarchy.bones.iter().filter(|b| b.name == *name);
        let bone = candidates.next()?;
        if candidates.next().is_some()
            || bone
                .default_transform
                .iter()
                .chain(&bone.pivot)
                .any(|v| !v.is_finite())
            || (0..3).any(|r| {
                bone.default_transform[r].abs() > 0.002
                    || bone.default_transform[r + 3].abs() > 0.002
                    || (bone.default_transform[r + 6] - 1.).abs() > 0.002
                    || (bone.pivot[r] - if r == 1 { 24. } else { 0. }).abs() > 0.002
            })
        {
            return None;
        }
        let m = &bone.matrix;
        if m.iter().any(|v| !v.is_finite())
            || [3, 7, 11].iter().any(|r| m[*r].abs() > 0.0001)
            || (m[15] - 1.).abs() > 0.0001
            || (0..3).any(|r| m[12 + r].abs() > 4096.)
        {
            return None;
        }
        let lengths: [f32; 3] =
            std::array::from_fn(|c| (0..3).map(|r| m[c * 4 + r].powi(2)).sum::<f32>().sqrt());
        if lengths.iter().any(|v| !(0.05..=8.).contains(v)) {
            return None;
        }
        for a in 0..3 {
            for b in a + 1..3 {
                let dot: f32 = (0..3).map(|r| m[a * 4 + r] * m[b * 4 + r]).sum();
                if dot.abs() > 0.001 * lengths[a] * lengths[b] {
                    return None;
                }
            }
        }
        let det = m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2])
            + m[8] * (m[1] * m[6] - m[5] * m[2]);
        if (det / (lengths[0] * lengths[1] * lengths[2]) - 1.).abs() > 0.001 {
            return None;
        }
        if i != 0 {
            for r in 0..3 {
                for c in 0..3 {
                    wings[i - 1][r * 4 + c] = m[c * 4 + r];
                }
                wings[i - 1][r * 4 + 3] = m[12 + r] / 16.;
            }
        }
    }
    Some(WingPose {
        geometry: geometry.into(),
        hierarchy: selected,
        animation_frame: pose.update_frame,
        wings,
    })
}
