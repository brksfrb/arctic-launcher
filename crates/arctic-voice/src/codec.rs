//! Opus in and out: 48 kHz mono, 20 ms frames, tuned for speech.

use opus::{Application, Bitrate, Channels, Decoder, Encoder};

use crate::{FRAME, SAMPLE_RATE, packet::MAX_OPUS};

/// Clear speech without much traffic (about 3 KB/s per speaker).
const BITRATE: i32 = 24_000;

pub struct VoiceEncoder {
    inner: Encoder,
    buf: Vec<u8>,
}

impl VoiceEncoder {
    pub fn new() -> Result<Self, opus::Error> {
        let mut inner = Encoder::new(SAMPLE_RATE, Channels::Mono, Application::Voip)?;
        inner.set_bitrate(Bitrate::Bits(BITRATE))?;
        inner.set_inband_fec(true)?;
        inner.set_packet_loss_perc(10)?;
        Ok(Self {
            inner,
            buf: vec![0; MAX_OPUS],
        })
    }

    /// One 20 ms frame ([`FRAME`] samples) to an Opus packet.
    pub fn encode(&mut self, frame: &[f32]) -> Result<Vec<u8>, opus::Error> {
        let n = self
            .inner
            .encode_float(&frame[..FRAME.min(frame.len())], &mut self.buf)?;
        Ok(self.buf[..n].to_vec())
    }
}

pub struct VoiceDecoder {
    inner: Decoder,
}

impl VoiceDecoder {
    pub fn new() -> Result<Self, opus::Error> {
        Ok(Self {
            inner: Decoder::new(SAMPLE_RATE, Channels::Mono)?,
        })
    }

    /// A packet to 20 ms of samples; `None` fills in a lost packet.
    pub fn decode(&mut self, packet: Option<&[u8]>) -> Result<Vec<f32>, opus::Error> {
        let mut out = vec![0.0f32; FRAME];
        let n = self
            .inner
            .decode_float(packet.unwrap_or(&[]), &mut out, false)?;
        out.truncate(n);
        out.resize(FRAME, 0.0);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_survives_the_round_trip() {
        let mut enc = VoiceEncoder::new().unwrap();
        let mut dec = VoiceDecoder::new().unwrap();
        let tone: Vec<f32> = (0..FRAME)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / SAMPLE_RATE as f32).sin() * 0.5)
            .collect();
        let mut energy = 0.0;
        // A few frames for the codec to settle.
        for _ in 0..5 {
            let packet = enc.encode(&tone).unwrap();
            assert!(!packet.is_empty() && packet.len() < 200);
            let out = dec.decode(Some(&packet)).unwrap();
            assert_eq!(out.len(), FRAME);
            energy = out.iter().map(|s| s * s).sum::<f32>() / FRAME as f32;
        }
        assert!(energy > 0.05, "decoded tone too quiet: {energy}");
        // A lost packet still gives a frame (concealment).
        assert_eq!(dec.decode(None).unwrap().len(), FRAME);
    }
}
