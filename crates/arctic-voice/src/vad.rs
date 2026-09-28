//! Voice activation: talk when the microphone is louder than a threshold,
//! and keep going a moment after (so word endings aren't cut).

/// Keep sending this many frames after the voice drops (300 ms).
const HANGOVER: u32 = 15;

pub struct Vad {
    /// Loudness (dBFS) that counts as talking, like -45.
    pub threshold_db: f32,
    hang: u32,
}

impl Vad {
    pub fn new(threshold_db: f32) -> Self {
        Self {
            threshold_db,
            hang: 0,
        }
    }

    /// Is this frame speech (or the tail of it)?
    pub fn speaking(&mut self, frame: &[f32]) -> bool {
        if level_db(frame) >= self.threshold_db {
            self.hang = HANGOVER;
            true
        } else if self.hang > 0 {
            self.hang -= 1;
            true
        } else {
            false
        }
    }
}

/// A frame's loudness in dBFS (RMS; silence is -100).
pub fn level_db(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return -100.0;
    }
    let rms = (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt();
    if rms <= 1e-5 {
        -100.0
    } else {
        20.0 * rms.log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_with_a_tail() {
        let mut v = Vad::new(-40.0);
        let quiet = vec![0.001f32; 960];
        let loud = vec![0.2f32; 960];
        assert!(!v.speaking(&quiet));
        assert!(v.speaking(&loud));
        for _ in 0..HANGOVER {
            assert!(v.speaking(&quiet));
        }
        assert!(!v.speaking(&quiet));
        assert!((level_db(&[1.0; 10]) - 0.0).abs() < 1e-3);
        assert_eq!(level_db(&[0.0; 10]), -100.0);
    }
}
