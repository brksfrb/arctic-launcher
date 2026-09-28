//! One voice packet on the wire: a version byte, a sequence number (so
//! lost and reordered packets are noticed) and one Opus frame.

/// Bumped when the format changes; other versions are ignored.
pub const VERSION: u8 = 1;
const HEADER: usize = 3;
/// Opus frames are far smaller; anything bigger is not ours.
pub const MAX_OPUS: usize = 1275;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub seq: u16,
    pub opus: Vec<u8>,
}

impl Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER + self.opus.len());
        out.push(VERSION);
        out.extend_from_slice(&self.seq.to_be_bytes());
        out.extend_from_slice(&self.opus);
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() <= HEADER || bytes.len() > HEADER + MAX_OPUS || bytes[0] != VERSION {
            return None;
        }
        Some(Self {
            seq: u16::from_be_bytes([bytes[1], bytes[2]]),
            opus: bytes[HEADER..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_rejects_junk() {
        let p = Packet {
            seq: 65_535,
            opus: vec![1, 2, 3],
        };
        assert_eq!(Packet::decode(&p.encode()), Some(p));
        assert_eq!(Packet::decode(&[VERSION, 0, 1]), None);
        assert_eq!(Packet::decode(&[9, 0, 1, 5]), None);
        assert_eq!(Packet::decode(&vec![VERSION; HEADER + MAX_OPUS + 1]), None);
    }
}
