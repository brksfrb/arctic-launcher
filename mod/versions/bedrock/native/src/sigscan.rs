//! Byte-signature scanning for version-tolerant engine bindings.
//!
//! A signature is a run of hex bytes with `?` wildcards (IDA style, as used by
//! LatiteClient's `src/mc/Addresses.h`). Wildcards cover what a minor game
//! update moves (relative call/data displacements, stack frame sizes, field
//! offsets), so a binding keeps resolving across patch releases instead of
//! being gated on one PE timestamp or `.text` hash. Every lookup reports
//! whether it was found, missing or ambiguous, so a failing hook can be named
//! in the adapter log without the game binary.

/// One parsed signature: `Some(byte)` must match, `None` matches anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern(Vec<Option<u8>>);

impl Pattern {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut bytes = Vec::new();
        for part in text.split_whitespace() {
            if part == "?" || part == "??" {
                bytes.push(None);
            } else if part.len() == 2 {
                bytes.push(Some(
                    u8::from_str_radix(part, 16)
                        .map_err(|_| format!("invalid signature byte {part:?}"))?,
                ));
            } else {
                return Err(format!("invalid signature byte {part:?}"));
            }
        }
        // A pattern must anchor on real bytes at both ends, or a match is meaningless.
        match (bytes.first(), bytes.last()) {
            (Some(Some(_)), Some(Some(_))) => Ok(Self(bytes)),
            _ => Err("signature must start and end with a concrete byte".into()),
        }
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn matches_at(&self, data: &[u8], at: usize) -> bool {
        data.get(at..at + self.0.len()).is_some_and(|window| {
            window
                .iter()
                .zip(&self.0)
                .all(|(byte, want)| want.is_none_or(|w| w == *byte))
        })
    }

    /// Offsets of the first `limit` matches in `data`.
    pub fn find(&self, data: &[u8], limit: usize) -> Vec<usize> {
        let mut hits = Vec::new();
        let Some(Some(first)) = self.0.first().copied() else {
            return hits;
        };
        if data.len() < self.0.len() {
            return hits;
        }
        let last_start = data.len() - self.0.len();
        let mut from = 0;
        while from <= last_start && hits.len() < limit {
            // memchr-style skip to the anchor byte keeps a 240 MB scan fast.
            let Some(step) = data[from..=last_start].iter().position(|b| *b == first) else {
                break;
            };
            let at = from + step;
            if self.matches_at(data, at) {
                hits.push(at);
            }
            from = at + 1;
        }
        hits
    }
}

/// How a match becomes the address a binding uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolve {
    /// The match is the function start.
    Direct,
    /// A rel32 at `match + field` relative to `match + end` (a call target or
    /// a RIP-relative data address).
    Relative { field: usize, end: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Offset (from the start of the scanned data) of the resolved address.
    Found {
        matched: usize,
        resolved: usize,
    },
    Missing,
    Ambiguous(usize),
}

/// Scan `data` (one image section, mapped at its RVA) for a unique match and
/// resolve it. `section_rva` turns offsets into RVAs; resolved addresses are
/// returned as RVAs and may point outside the scanned section (data globals).
pub fn resolve(pattern: &Pattern, how: Resolve, data: &[u8], section_rva: usize) -> Outcome {
    let hits = pattern.find(data, 2);
    match hits.as_slice() {
        [] => Outcome::Missing,
        [at] => {
            let resolved = match how {
                Resolve::Direct => section_rva + at,
                Resolve::Relative { field, end } => {
                    let Some(bytes) = data.get(at + field..at + field + 4) else {
                        return Outcome::Missing;
                    };
                    let displacement = i32::from_le_bytes(bytes.try_into().expect("four bytes"));
                    let next = (section_rva + at + end) as i64;
                    let target = next + i64::from(displacement);
                    if target < 0 {
                        return Outcome::Missing;
                    }
                    target as usize
                }
            };
            Outcome::Found {
                matched: section_rva + at,
                resolved,
            }
        }
        _ => Outcome::Ambiguous(pattern.find(data, 64).len()),
    }
}

/// A PE section as mapped in memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub rva: usize,
    pub size: usize,
    pub executable: bool,
}

/// Parse the section table from a mapped PE32+ header page.
pub fn sections(header: &[u8]) -> Result<Vec<Section>, String> {
    let u16_at = |at: usize| -> Result<u16, String> {
        Ok(u16::from_le_bytes(
            header
                .get(at..at + 2)
                .ok_or("short PE header")?
                .try_into()
                .expect("two bytes"),
        ))
    };
    let u32_at = |at: usize| -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            header
                .get(at..at + 4)
                .ok_or("short PE header")?
                .try_into()
                .expect("four bytes"),
        ))
    };
    if header.get(..2) != Some(b"MZ") {
        return Err("not a PE image".into());
    }
    let pe = u32_at(60)? as usize;
    if pe > 2048 || header.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err("no PE signature".into());
    }
    if u16_at(pe + 4)? != 0x8664 || u16_at(pe + 24)? != 0x20b {
        return Err("not an x64 PE32+ image".into());
    }
    let count = u16_at(pe + 6)? as usize;
    let optional = u16_at(pe + 20)? as usize;
    if count > 96 || optional > 1024 {
        return Err("invalid section table".into());
    }
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let at = pe + 24 + optional + i * 40;
        let raw = header.get(at..at + 8).ok_or("short section table")?;
        let name = String::from_utf8_lossy(raw)
            .trim_end_matches('\0')
            .to_owned();
        let virtual_size = u32_at(at + 8)? as usize;
        let rva = u32_at(at + 12)? as usize;
        let raw_size = u32_at(at + 16)? as usize;
        let characteristics = u32_at(at + 36)?;
        result.push(Section {
            name,
            rva,
            size: virtual_size.max(raw_size),
            executable: characteristics & 0x2000_0000 != 0,
        });
    }
    Ok(result)
}

/// Read an MSVC `std::string` (32 bytes: 16-byte SSO buffer or pointer, size,
/// capacity) given its raw bytes and a reader for heap storage. Returns None
/// when the layout is implausible, so callers can calibrate offsets safely.
pub fn msvc_string(
    raw: &[u8; 32],
    max: usize,
    read: impl Fn(usize, usize) -> Option<Vec<u8>>,
) -> Option<String> {
    let size = u64::from_le_bytes(raw[16..24].try_into().ok()?) as usize;
    let capacity = u64::from_le_bytes(raw[24..32].try_into().ok()?) as usize;
    if size == 0 || size > max || capacity < size || !(15..=(1 << 30)).contains(&capacity) {
        return None;
    }
    let bytes = if capacity < 16 {
        raw[..size].to_vec()
    } else {
        let pointer = u64::from_le_bytes(raw[..8].try_into().ok()?) as usize;
        if pointer < 0x10000 {
            return None;
        }
        read(pointer, size)?
    };
    String::from_utf8(bytes).ok()
}

/// Screen/control names are short lowercase identifiers ("hud_screen").
pub fn plausible_name(name: &str) -> bool {
    (3..=96).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
        && name.bytes().any(|b| b.is_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wildcards_and_rejects_loose_ends() {
        let p = Pattern::parse("E8 ? ? ? ? 90").unwrap();
        assert_eq!(p.len(), 6);
        assert!(Pattern::parse("? E8").is_err());
        assert!(Pattern::parse("E8 ?").is_err());
        assert!(Pattern::parse("E8 XYZ").is_err());
        assert!(Pattern::parse("").is_err());
    }

    #[test]
    fn finds_unique_direct_match() {
        let data = [0x00, 0x55, 0x41, 0x57, 0x99, 0x55, 0x41, 0x56];
        let p = Pattern::parse("55 41 57").unwrap();
        assert_eq!(
            resolve(&p, Resolve::Direct, &data, 0x1000),
            Outcome::Found {
                matched: 0x1001,
                resolved: 0x1001
            }
        );
        let p = Pattern::parse("55 41 ? ").unwrap_err();
        assert!(p.contains("concrete"));
    }

    #[test]
    fn reports_missing_and_ambiguous() {
        let data = [0x55, 0x41, 0x55, 0x41, 0x55, 0x41];
        assert_eq!(
            resolve(&Pattern::parse("55 41").unwrap(), Resolve::Direct, &data, 0),
            Outcome::Ambiguous(3)
        );
        assert_eq!(
            resolve(&Pattern::parse("41 41").unwrap(), Resolve::Direct, &data, 0),
            Outcome::Missing
        );
    }

    #[test]
    fn resolves_call_and_rip_relative_targets() {
        // call +0x10 at offset 2: target = 0x2000 + 2 + 5 + 0x10
        let mut data = vec![0xCC, 0xCC, 0xE8, 0x10, 0, 0, 0, 0x90, 0x48];
        let p = Pattern::parse("E8 ? ? ? ? 90 48").unwrap();
        assert_eq!(
            resolve(&p, Resolve::Relative { field: 1, end: 5 }, &data, 0x2000),
            Outcome::Found {
                matched: 0x2002,
                resolved: 0x2000 + 2 + 5 + 0x10
            }
        );
        // Negative displacement.
        data[3..7].copy_from_slice(&(-8i32).to_le_bytes());
        assert_eq!(
            resolve(&p, Resolve::Relative { field: 1, end: 5 }, &data, 0x2000),
            Outcome::Found {
                matched: 0x2002,
                resolved: 0x2000 + 2 + 5 - 8
            }
        );
        // mov [rip+disp], edx (89 15 disp32), the MouseDevice global form.
        let data = [0x89, 0x15, 0x00, 0x01, 0, 0, 0xC7, 0x47];
        let p = Pattern::parse("89 15 ? ? ? ? C7 47").unwrap();
        assert_eq!(
            resolve(&p, Resolve::Relative { field: 2, end: 6 }, &data, 0x10),
            Outcome::Found {
                matched: 0x10,
                resolved: 0x10 + 6 + 0x100
            }
        );
    }

    #[test]
    fn every_shipped_signature_parses() {
        for (name, text, _, _) in crate::core_signatures::SIGNATURES {
            assert!(Pattern::parse(text).is_ok(), "{name}");
        }
    }

    #[test]
    fn reads_section_table() {
        let mut header = vec![0u8; 1024];
        header[..2].copy_from_slice(b"MZ");
        header[60..64].copy_from_slice(&128u32.to_le_bytes());
        header[128..132].copy_from_slice(b"PE\0\0");
        header[132..134].copy_from_slice(&0x8664u16.to_le_bytes());
        header[134..136].copy_from_slice(&2u16.to_le_bytes());
        header[148..150].copy_from_slice(&240u16.to_le_bytes());
        header[152..154].copy_from_slice(&0x20bu16.to_le_bytes());
        let table = 128 + 24 + 240;
        header[table..table + 5].copy_from_slice(b".text");
        header[table + 8..table + 12].copy_from_slice(&0x5000u32.to_le_bytes());
        header[table + 12..table + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        header[table + 36..table + 40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        header[table + 40..table + 46].copy_from_slice(b".rdata");
        header[table + 48..table + 52].copy_from_slice(&0x100u32.to_le_bytes());
        header[table + 52..table + 56].copy_from_slice(&0x6000u32.to_le_bytes());
        let s = sections(&header).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(
            (s[0].name.as_str(), s[0].rva, s[0].size, s[0].executable),
            (".text", 0x1000, 0x5000, true)
        );
        assert_eq!((s[1].name.as_str(), s[1].executable), (".rdata", false));
    }

    #[test]
    fn reads_msvc_strings() {
        let mut raw = [0u8; 32];
        raw[..10].copy_from_slice(b"hud_screen");
        raw[16..24].copy_from_slice(&10u64.to_le_bytes());
        raw[24..32].copy_from_slice(&15u64.to_le_bytes());
        assert_eq!(
            msvc_string(&raw, 64, |_, _| None).as_deref(),
            Some("hud_screen")
        );
        let mut heap = [0u8; 32];
        heap[..8].copy_from_slice(&0x7000_0000u64.to_le_bytes());
        heap[16..24].copy_from_slice(&20u64.to_le_bytes());
        heap[24..32].copy_from_slice(&31u64.to_le_bytes());
        let text = msvc_string(&heap, 64, |at, len| {
            (at == 0x7000_0000).then(|| b"start_screen_content".to_vec()[..len].to_vec())
        });
        assert_eq!(text.as_deref(), Some("start_screen_content"));
        // Garbage sizes are rejected, not read.
        heap[16..24].copy_from_slice(&5000u64.to_le_bytes());
        assert!(msvc_string(&heap, 64, |_, _| panic!("must not read")).is_none());
        assert!(plausible_name("hud_screen"));
        assert!(!plausible_name("HUD screen"));
        assert!(!plausible_name("__"));
    }
}
