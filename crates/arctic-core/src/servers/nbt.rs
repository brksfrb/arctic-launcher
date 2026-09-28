//! A small, bounded NBT reader and writer: enough for `servers.dat` (an
//! uncompressed compound holding a list of server compounds). Whatever is
//! read is written back the same, so entries the game added stay intact.

const MAX_DEPTH: usize = 32;
/// Longest list or array we accept (servers.dat has a few dozen entries).
const MAX_LEN: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Bytes(Vec<u8>),
    String(String),
    List(Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    Ints(Vec<i32>),
    Longs(Vec<i64>),
}

impl Tag {
    pub fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Tag::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Tag]> {
        match self {
            Tag::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_byte(&self) -> Option<i8> {
        match self {
            Tag::Byte(b) => Some(*b),
            _ => None,
        }
    }
}

/// The root compound of an uncompressed NBT file.
pub fn read_root(data: &[u8]) -> Option<Tag> {
    let mut r = Reader { data, at: 0 };
    if r.u8()? != 10 {
        return None;
    }
    r.string()?;
    r.payload(10, 0)
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let end = self.at.checked_add(n)?;
        let bytes = self.data.get(self.at..end)?;
        self.at = end;
        Some(bytes)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.array::<1>()?[0])
    }

    fn len(&mut self) -> Option<usize> {
        let n = i32::from_be_bytes(self.array()?);
        usize::try_from(n).ok().filter(|n| *n <= MAX_LEN)
    }

    /// Java's modified UTF-8; read as UTF-8, which matches it for everything
    /// but NUL and characters outside the Basic Multilingual Plane.
    fn string(&mut self) -> Option<String> {
        let len = u16::from_be_bytes(self.array()?) as usize;
        Some(String::from_utf8_lossy(self.take(len)?).into_owned())
    }

    fn payload(&mut self, kind: u8, depth: usize) -> Option<Tag> {
        if depth > MAX_DEPTH {
            return None;
        }
        Some(match kind {
            1 => Tag::Byte(i8::from_be_bytes(self.array()?)),
            2 => Tag::Short(i16::from_be_bytes(self.array()?)),
            3 => Tag::Int(i32::from_be_bytes(self.array()?)),
            4 => Tag::Long(i64::from_be_bytes(self.array()?)),
            5 => Tag::Float(f32::from_be_bytes(self.array()?)),
            6 => Tag::Double(f64::from_be_bytes(self.array()?)),
            7 => {
                let n = self.len()?;
                Tag::Bytes(self.take(n)?.to_vec())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let item = self.u8()?;
                let n = self.len()?;
                let mut items = Vec::with_capacity(n.min(256));
                for _ in 0..n {
                    items.push(self.payload(item, depth + 1)?);
                }
                Tag::List(items)
            }
            10 => {
                let mut entries = Vec::new();
                loop {
                    let kind = self.u8()?;
                    if kind == 0 {
                        break;
                    }
                    let name = self.string()?;
                    entries.push((name, self.payload(kind, depth + 1)?));
                }
                Tag::Compound(entries)
            }
            11 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(256));
                for _ in 0..n {
                    v.push(i32::from_be_bytes(self.array()?));
                }
                Tag::Ints(v)
            }
            12 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(256));
                for _ in 0..n {
                    v.push(i64::from_be_bytes(self.array()?));
                }
                Tag::Longs(v)
            }
            _ => return None,
        })
    }
}

/// An uncompressed NBT file with `root` as its (unnamed) root compound.
pub fn write_root(root: &Tag) -> Vec<u8> {
    let mut out = vec![10, 0, 0];
    write_payload(&mut out, root);
    out
}

impl Tag {
    fn kind(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::Bytes(_) => 7,
            Tag::String(_) => 8,
            Tag::List(_) => 9,
            Tag::Compound(_) => 10,
            Tag::Ints(_) => 11,
            Tag::Longs(_) => 12,
        }
    }
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    // Longer strings can't be stored; servers.dat never has them.
    let bytes = &s.as_bytes()[..s.len().min(u16::MAX as usize)];
    out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn write_len(out: &mut Vec<u8>, n: usize) {
    out.extend_from_slice(&(n as i32).to_be_bytes());
}

fn write_payload(out: &mut Vec<u8>, tag: &Tag) {
    match tag {
        Tag::Byte(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Bytes(v) => {
            write_len(out, v.len());
            out.extend_from_slice(v);
        }
        Tag::String(v) => write_string(out, v),
        Tag::List(items) => {
            out.push(items.first().map_or(0, Tag::kind));
            write_len(out, items.len());
            for item in items {
                write_payload(out, item);
            }
        }
        Tag::Compound(entries) => {
            for (name, value) in entries {
                out.push(value.kind());
                write_string(out, name);
                write_payload(out, value);
            }
            out.push(0);
        }
        Tag::Ints(v) => {
            write_len(out, v.len());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        Tag::Longs(v) => {
            write_len(out, v.len());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
    }
}

/// Test helper: write `servers.dat` for these (name, ip, hidden) entries.
#[cfg(test)]
pub(crate) fn fake_servers_dat(entries: &[(&str, &str, bool)]) -> Vec<u8> {
    fn string(out: &mut Vec<u8>, s: &str) {
        out.extend_from_slice(&(s.len() as u16).to_be_bytes());
        out.extend_from_slice(s.as_bytes());
    }
    let mut out = vec![10, 0, 0, 9];
    string(&mut out, "servers");
    out.push(10);
    out.extend_from_slice(&(entries.len() as i32).to_be_bytes());
    for (name, ip, hidden) in entries {
        out.push(8);
        string(&mut out, "name");
        string(&mut out, name);
        out.push(8);
        string(&mut out, "ip");
        string(&mut out, ip);
        if *hidden {
            out.push(1);
            string(&mut out, "hidden");
            out.push(1);
        }
        out.push(0);
    }
    out.push(0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_back_what_it_read() {
        let root = Tag::Compound(vec![
            (
                "servers".into(),
                Tag::List(vec![Tag::Compound(vec![
                    ("name".into(), Tag::String("Café".into())),
                    ("ip".into(), Tag::String("a.b".into())),
                    ("acceptTextures".into(), Tag::Byte(1)),
                ])]),
            ),
            ("n".into(), Tag::Longs(vec![-1, 2])),
            ("i".into(), Tag::Ints(vec![7])),
            ("b".into(), Tag::Bytes(vec![1, 2, 3])),
            ("f".into(), Tag::Double(0.5)),
            ("empty".into(), Tag::List(Vec::new())),
        ]);
        let bytes = write_root(&root);
        assert_eq!(read_root(&bytes), Some(root));
        let fake = fake_servers_dat(&[("A", "a.net", true)]);
        assert_eq!(write_root(&read_root(&fake).unwrap()), fake);
    }
}
