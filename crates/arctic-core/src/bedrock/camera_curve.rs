//! Pure camera evaluation shared by the editor and indexed export feeder.
//! Mirrors Java CameraPath and the pinned offline helper's mixed easing rules.
use super::{CameraPath, CameraPose, CameraSampleTime};
use crate::{Error, Result};

#[derive(Clone, Copy)]
enum Ease {
    Linear,
    Smooth,
    Hold,
    Java,
}
#[derive(Clone, Copy)]
struct Key {
    time: u32,
    pose: CameraPose,
    ease: Ease,
}

/// Prepared, owned curve. Sampling never changes saved keyframe angles or
/// re-sorts times, and does not read an engine, clock, process or account.
pub struct CameraCurve {
    keys: Vec<Key>,
    yaw: Vec<f32>,
}

impl CameraPath {
    pub fn curve(&self) -> Result<CameraCurve> {
        self.validate()?;
        let keys: Vec<Key> = self
            .keyframes
            .iter()
            .map(|key| Key {
                time: key.time_ms,
                pose: key.pose,
                ease: match key.ease.as_str() {
                    "linear" => Ease::Linear,
                    "smooth" => Ease::Smooth,
                    "hold" => Ease::Hold,
                    "catmull_rom" => Ease::Java,
                    _ => unreachable!(),
                },
            })
            .collect();
        let mut yaw = Vec::with_capacity(keys.len());
        yaw.push(keys[0].pose.yaw as f32);
        for key in keys.iter().skip(1) {
            let before = *yaw.last().unwrap();
            let delta = key.pose.yaw as f32 - before;
            // Java Math.round(float): nearest integer with ties toward +inf.
            // Preserve each float operation; f32::round uses a different tie.
            let turns = ((delta / 360.0) as f64 + 0.5).floor() as f32;
            let delta = delta - 360.0f32 * turns;
            yaw.push(before + delta);
        }
        Ok(CameraCurve { keys, yaw })
    }
}

impl CameraCurve {
    fn endpoint(&self, index: usize) -> CameraPose {
        let mut p = self.keys[index].pose;
        if matches!(self.keys[index].ease, Ease::Java)
            || index > 0 && matches!(self.keys[index - 1].ease, Ease::Java)
        {
            p.yaw = self.yaw[index] as f64;
            p.pitch = p.pitch as f32 as f64;
            p.fov = p.fov as f32 as f64;
        }
        p
    }
    /// Exact fractional-millisecond export time, using the helper's bounds.
    pub fn sample_at(&self, time: CameraSampleTime) -> Result<CameraPose> {
        if time.time_ms > 86_400_000
            || time.denominator == 0
            || time.denominator > 1000
            || time.remainder >= time.denominator
            || time.time_ms == 86_400_000 && time.remainder != 0
        {
            return Err(Error::Other("invalid camera sample time".into()));
        }
        self.sample(time.time_ms as f64 + time.remainder as f64 / time.denominator as f64)
    }
    /// Java ReplayViewer passes CameraPath's double spline result through a
    /// float camera interface before it reaches the game. Keep this separate
    /// from raw curve sampling used by the editor. In particular, rounding an
    /// unwrapped yaw after normalization changes the applied Java angle.
    pub fn sample_for_view_at(&self, time: CameraSampleTime) -> Result<CameraPose> {
        let mut pose = self.sample_at(time)?;
        let millis = time.time_ms as f64 + time.remainder as f64 / time.denominator as f64;
        let index = self
            .keys
            .partition_point(|key| key.time as f64 <= millis)
            .saturating_sub(1);
        if matches!(self.keys[index].ease, Ease::Java) {
            pose.yaw = pose.yaw as f32 as f64;
            pose.pitch = pose.pitch as f32 as f64;
            pose.fov = pose.fov as f32 as f64;
        }
        Ok(pose)
    }
    pub fn sample(&self, time_ms: f64) -> Result<CameraPose> {
        if !time_ms.is_finite() || !(0.0..=86_400_000.0).contains(&time_ms) {
            return Err(Error::Other("invalid camera sample time".into()));
        }
        if time_ms <= self.keys[0].time as f64 {
            return Ok(self.endpoint(0));
        }
        let i = self.keys.partition_point(|key| key.time as f64 <= time_ms) - 1;
        if i + 1 == self.keys.len() {
            return Ok(self.endpoint(i));
        }
        let a = self.keys[i];
        let b = self.keys[i + 1];
        let mut t = (time_ms - a.time as f64) / (b.time - a.time) as f64;
        if matches!(a.ease, Ease::Java) {
            let left = i.saturating_sub(1);
            let right = (i + 2).min(self.keys.len() - 1);
            let p0 = self.keys[left].pose;
            let p3 = self.keys[right].pose;
            let p = CameraPose {
                x: spline(p0.x, a.pose.x, b.pose.x, p3.x, t),
                y: spline(p0.y, a.pose.y, b.pose.y, p3.y, t),
                z: spline(p0.z, a.pose.z, b.pose.z, p3.z, t),
                yaw: spline(
                    self.yaw[left] as f64,
                    self.yaw[i] as f64,
                    self.yaw[i + 1] as f64,
                    self.yaw[right] as f64,
                    t,
                ),
                pitch: spline(
                    p0.pitch as f32 as f64,
                    a.pose.pitch as f32 as f64,
                    b.pose.pitch as f32 as f64,
                    p3.pitch as f32 as f64,
                    t,
                )
                .clamp(-90.0, 90.0),
                fov: a.pose.fov as f32 as f64 + (b.pose.fov as f32 - a.pose.fov as f32) as f64 * t,
            };
            // Preserve Java's unwrapped track even when repeated turns carry
            // its evaluated yaw beyond the saved-key bound. World/physical
            // overshoot remains invalid; normalization happens at wire_pose.
            p.validate_evaluated()?;
            return Ok(p);
        }
        if matches!(a.ease, Ease::Smooth) {
            t = t * t * (3.0 - 2.0 * t);
        }
        if matches!(a.ease, Ease::Hold) {
            t = 0.0;
        }
        let mix = |x: f64, y: f64| x + (y - x) * t;
        let delta = ((b.pose.yaw - a.pose.yaw) % 360.0 + 540.0) % 360.0 - 180.0;
        Ok(CameraPose {
            x: mix(a.pose.x, b.pose.x),
            y: mix(a.pose.y, b.pose.y),
            z: mix(a.pose.z, b.pose.z),
            yaw: a.pose.yaw + delta * t,
            pitch: mix(a.pose.pitch, b.pose.pitch),
            fov: mix(a.pose.fov, b.pose.fov),
        })
    }
}

fn spline(p0: f64, p1: f64, p2: f64, p3: f64, u: f64) -> f64 {
    let u2 = u * u;
    let u3 = u2 * u;
    0.5 * (2.0 * p1
        + (-p0 + p2) * u
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * u3)
}

impl CameraPose {
    /// Protocol float32 pose, including equivalent-yaw normalization before
    /// rounding. Java curves must first use sample_for_view_at to preserve
    /// ReplayViewer's float camera argument conversion. An angle rounded to
    /// +180 uses -180, as in camera.cjs.
    pub fn wire_pose(self) -> Result<Self> {
        self.validate_evaluated()?;
        let yaw = (((self.yaw + 180.0) % 360.0 + 360.0) % 360.0 - 180.0) as f32;
        let pose = Self {
            x: self.x as f32 as f64,
            y: self.y as f32 as f64,
            z: self.z as f32 as f64,
            yaw: if yaw == 180.0 { -180.0 } else { yaw as f64 },
            pitch: self.pitch as f32 as f64,
            fov: self.fov as f32 as f64,
        };
        pose.validate()?;
        Ok(pose)
    }
}
