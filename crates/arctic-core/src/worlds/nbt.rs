//! Just enough NBT to read a world's display name from `level.dat`
//! (gzip-compressed): the `LevelName` string tag.

use std::io::Read;

/// Largest level.dat we decompress.
const MAX_LEVEL_DAT: u64 = 8 * 1024 * 1024;
const TAG_STRING: u8 = 8;

/// `LevelName` from a (gzipped) level.dat.
pub fn level_name(gzipped: &[u8]) -> Option<String> {
    let mut data = Vec::new();
    flate2::read::GzDecoder::new(gzipped)
        .take(MAX_LEVEL_DAT)
        .read_to_end(&mut data)
        .ok()?;
    find_string(&data, "LevelName")
}

/// The value of the first string tag called `key`. Scanning for the tag
/// header is enough here: a string tag is `08`, a big-endian name length,
/// the name, then a big-endian value length and the value.
fn find_string(data: &[u8], key: &str) -> Option<String> {
    let mut needle = vec![TAG_STRING];
    needle.extend_from_slice(&(key.len() as u16).to_be_bytes());
    needle.extend_from_slice(key.as_bytes());
    let at = data
        .windows(needle.len())
        .position(|w| w == needle.as_slice())?;
    let start = at + needle.len();
    let len = u16::from_be_bytes([*data.get(start)?, *data.get(start + 1)?]) as usize;
    let value = data.get(start + 2..start + 2 + len)?;
    String::from_utf8(value.to_vec()).ok()
}

#[cfg(test)]
pub(crate) fn fake_level_dat(name: &str) -> Vec<u8> {
    use std::io::Write;
    // Root compound { Data: compound { LevelName: string } }
    let mut nbt = vec![10, 0, 0, 10, 0, 4];
    nbt.extend_from_slice(b"Data");
    nbt.push(TAG_STRING);
    nbt.extend_from_slice(&9u16.to_be_bytes());
    nbt.extend_from_slice(b"LevelName");
    nbt.extend_from_slice(&(name.len() as u16).to_be_bytes());
    nbt.extend_from_slice(name.as_bytes());
    nbt.extend_from_slice(&[0, 0]);
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&nbt).unwrap();
    gz.finish().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_level_name() {
        assert_eq!(
            level_name(&fake_level_dat("My World")).as_deref(),
            Some("My World")
        );
        assert_eq!(level_name(b"not gzip"), None);
    }

    #[test]
    fn truncated_data_is_none() {
        let mut data = vec![TAG_STRING, 0, 9];
        data.extend_from_slice(b"LevelName");
        data.extend_from_slice(&[0, 50, b'x']);
        assert_eq!(find_string(&data, "LevelName"), None);
    }
}
