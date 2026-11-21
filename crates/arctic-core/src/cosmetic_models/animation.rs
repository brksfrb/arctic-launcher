//! The position and rotation tracks shared with the mod's Bedrock idle animations.

use std::collections::HashMap;

use serde::Deserialize;

use crate::{Error, Result};

const MAX_TRACKS: usize = 64;
const MAX_FRAMES: usize = 256;

type Vector = [f32; 3];

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Frame {
    Vector(Vector),
    Keyframe {
        post: Vector,
        #[serde(default)]
        lerp_mode: String,
    },
}

impl Frame {
    fn vector(&self) -> Vector {
        match self {
            Self::Vector(value) | Self::Keyframe { post: value, .. } => *value,
        }
    }

    fn mode(&self) -> &str {
        match self {
            Self::Vector(_) => "",
            Self::Keyframe { lerp_mode, .. } => lerp_mode,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Channel {
    Constant(Vector),
    Keyframes(HashMap<String, Frame>),
}

#[derive(Debug, Clone, Default)]
struct Track(Vec<(f32, Frame)>);

impl Track {
    fn parse(channel: Channel, limit: f32) -> Option<Self> {
        let mut frames = match channel {
            Channel::Constant(value) => vec![(0.0, Frame::Vector(value))],
            Channel::Keyframes(frames) => frames
                .into_iter()
                .map(|(time, frame)| Some((time.parse::<f32>().ok()?, frame)))
                .collect::<Option<Vec<_>>>()?,
        };
        if frames.is_empty()
            || frames.len() > MAX_FRAMES
            || frames.iter().any(|(t, frame)| {
                !(0.0..=60.0).contains(t) || frame.vector().iter().any(|v| v.abs() > limit)
            })
        {
            return None;
        }
        frames.sort_by(|a, b| a.0.total_cmp(&b.0));
        Some(Self(frames))
    }

    fn at(&self, t: f32) -> Vector {
        let frames = &self.0;
        let end = frames.partition_point(|(time, _)| *time < t);
        match end {
            0 => frames.first().map_or([0.0; 3], |(_, f)| f.vector()),
            n if n == frames.len() => frames[n - 1].1.vector(),
            n => {
                let (start, a) = &frames[n - 1];
                let (finish, b) = &frames[n];
                let p1 = a.vector();
                if a.mode() == "step" || b.mode() == "step" {
                    return p1;
                }
                let p2 = b.vector();
                let k = (t - start) / (finish - start);
                if a.mode() == "catmullrom" || b.mode() == "catmullrom" {
                    let p0 = frames[n.saturating_sub(2)].1.vector();
                    let p3 = frames[(n + 1).min(frames.len() - 1)].1.vector();
                    std::array::from_fn(|i| {
                        0.5 * (2.0 * p1[i]
                            + (p2[i] - p0[i]) * k
                            + (2.0 * p0[i] - 5.0 * p1[i] + 4.0 * p2[i] - p3[i]) * k * k
                            + (3.0 * (p1[i] - p2[i]) + p3[i] - p0[i]) * k * k * k)
                    })
                } else {
                    std::array::from_fn(|i| p1[i] + (p2[i] - p1[i]) * k)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Channels {
    position: Option<Channel>,
    rotation: Option<Channel>,
}

#[derive(Debug, Clone, Default)]
pub struct BoneMotion {
    pub position: Vector,
    pub rotation: Vector,
}

/// A cosmetic's idle animation, sampled in Java angles and Y-up offsets.
#[derive(Debug, Clone)]
pub struct Animation {
    length: f32,
    looping: bool,
    bones: HashMap<String, (Track, Track)>,
}

impl Animation {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        struct File {
            animations: serde_json::Map<String, serde_json::Value>,
        }

        #[derive(Deserialize)]
        struct Clip {
            animation_length: f32,
            #[serde(default, rename = "loop")]
            looping: bool,
            #[serde(default)]
            bones: HashMap<String, Channels>,
        }

        let bad = || Error::Other("invalid cosmetic animation".into());
        if bytes.len() as u64 > super::MAX_JSON_BYTES {
            return Err(bad());
        }
        let file: File = serde_json::from_slice(bytes).map_err(|_| bad())?;
        let (_, first) = file.animations.into_iter().next().ok_or_else(bad)?;
        let clip: Clip = serde_json::from_value(first).map_err(|_| bad())?;
        if clip.animation_length <= 0.0
            || clip.animation_length > 60.0
            || clip.bones.len() > MAX_TRACKS
        {
            return Err(bad());
        }
        let mut bones = HashMap::new();
        for (name, channels) in clip.bones {
            let parse = |channel, limit| match channel {
                Some(channel) => Track::parse(channel, limit).ok_or_else(bad),
                None => Ok(Track::default()),
            };
            bones.insert(
                name,
                (
                    parse(channels.position, 256.0)?,
                    parse(channels.rotation, 3600.0)?,
                ),
            );
        }
        Ok(Self {
            length: clip.animation_length,
            looping: clip.looping,
            bones,
        })
    }

    pub fn sample(&self, bone: &str, t: f64) -> BoneMotion {
        let Some((position, rotation)) = self.bones.get(bone) else {
            return BoneMotion::default();
        };
        let length = f64::from(self.length);
        let t = if self.looping {
            t.rem_euclid(length)
        } else {
            t.min(length)
        } as f32;
        BoneMotion {
            position: position.at(t),
            rotation: rotation.at(t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_a_looping_wing_about_its_signed_yaw() {
        let a = Animation::parse(
            br#"{"animations":{"idle":{"animation_length":4,"loop":true,
            "bones":{"wing_left":{"rotation":{"4":[0,0,0],"0":[0,0,0],"1":[0,10,2]}}}}}}"#,
        )
        .unwrap();
        assert_eq!(a.sample("wing_left", 4.5).rotation, [0.0, 5.0, 1.0]);
        assert_eq!(a.sample("wing_left", 4.5).position, [0.0; 3]);
        assert_eq!(a.sample("body", 4.5).rotation, [0.0; 3]);
    }
}
