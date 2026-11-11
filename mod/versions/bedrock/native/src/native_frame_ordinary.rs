//! Borrowed ordinary chunk mesh/index availability, matching the native producer.
//! A CPU fallback is accepted input, not a completed GPU upload.
use crate::native_frame_chunks::Copies;
use crate::world_hooks::read;
use std::{
    collections::HashSet,
    sync::atomic::{AtomicUsize, Ordering},
};

const RESOURCE_TABLE: usize = 0xe775460;
static PROFILE: AtomicUsize = AtomicUsize::new(0);

fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn canonical(address: usize) -> bool {
    (0x10000..0x0000_8000_0000_0000).contains(&address)
}

// Metadata only. The native mesh validator may use these CPU vectors while their
// backend handle is unavailable; requiring GPU handles here would change it.
fn cpu_vector(copies: &mut Copies, address: usize, stride: usize) -> Option<bool> {
    let header = copies.block(address, 24)?;
    let begin = word(&header, 0)?;
    let end = word(&header, 8)?;
    let capacity = word(&header, 16)?;
    if begin == 0 {
        return (end == 0 && capacity == 0).then_some(false);
    }
    if ![begin, end, capacity].into_iter().all(canonical) || !begin.is_multiple_of(4) {
        return None;
    }
    let length = end.checked_sub(begin)?;
    let allocated = capacity.checked_sub(begin)?;
    if end > capacity
        || !length.is_multiple_of(stride)
        || !allocated.is_multiple_of(stride)
        || allocated / stride > u32::MAX as usize
    {
        return None;
    }
    Some(length != 0)
}

fn client(
    copies: &mut Copies,
    address: usize,
    base: usize,
    owners: &mut HashSet<usize>,
    references: &mut usize,
) -> Option<bool> {
    // The common polymorphic ClientResourcePointer has a shared pair at +8/+16.
    // Its destructor and the Mesh copy constructor establish both offsets.
    let header = copies.block(address, 24)?;
    if word(&header, 0)? != base.checked_add(RESOURCE_TABLE)? {
        return None;
    }
    let block = word(&header, 8)?;
    let control = word(&header, 16)?;
    if block != 0 && (!canonical(block) || control == 0) {
        return None;
    }
    if control != 0 {
        copies.owner(control, owners)?;
        *references = (*references).checked_add(1)?;
    }
    if block == 0 {
        return Some(false);
    }
    // Preserve the producer's bit test. Other tracking bits are not bools and
    // the mutable validation counter at +0x10 is deliberately not copied.
    Some(copies.block(block.checked_add(0x12)?, 1)?[0] & 1 != 0)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sample {
    pub available: bool,
    pub cpu_positions: bool,
    pub cpu_indices: bool,
    pub attributes: bool,
    pub client_references: usize,
}
impl Sample {
    pub(crate) fn copy(
        object: usize,
        vertex: bool,
        copies: &mut Copies,
        owners: &mut HashSet<usize>,
    ) -> Option<Self> {
        let base = PROFILE.load(Ordering::Acquire);
        if base == 0 || crate::world_hooks::image_base() != Some(base) {
            return None;
        }
        let mut sample = Self {
            available: false,
            cpu_positions: false,
            cpu_indices: false,
            attributes: false,
            client_references: 0,
        };
        if !vertex {
            // An ordinary override index buffer has no CPU fallback in the
            // queue producer. The mesh's own index path below does have one.
            sample.available = client(copies, object, base, owners, &mut sample.client_references)?;
            return Some(sample);
        }
        let index_discriminator = u32::from_le_bytes(
            copies
                .block(object.checked_add(0x18)?, 4)?
                .try_into()
                .ok()?,
        );
        if !client(
            copies,
            object.checked_add(0x1b8)?,
            base,
            owners,
            &mut sample.client_references,
        )? {
            sample.cpu_positions = cpu_vector(copies, object.checked_add(0x60)?, 12)?;
            if !sample.cpu_positions {
                return Some(sample);
            }
        }
        // VertexLayout is opaque in the public declaration. Retain the actual
        // native equality-to-one test, without inventing a bool or LOD enum.
        sample.attributes = copies.block(object.checked_add(0x25c)?, 1)?[0] == 1;
        if sample.attributes
            && !client(
                copies,
                object.checked_add(0x1d0)?,
                base,
                owners,
                &mut sample.client_references,
            )?
        {
            return Some(sample);
        }
        if index_discriminator != 0
            && !client(copies, object, base, owners, &mut sample.client_references)?
        {
            sample.cpu_indices = cpu_vector(copies, object.checked_add(0xa8)?, 4)?;
            if !sample.cpu_indices {
                return Some(sample);
            }
        }
        sample.available = true;
        Some(sample)
    }
}

pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated ordinary frame-resource image".into());
    }
    let sites: &[(usize, &[u8])] = &[
        (
            0x4b04337,
            &[0x8b, 0x71, 0x18, 0x48, 0x8b, 0xb9, 0xc0, 0x01, 0, 0],
        ),
        (
            0x4b0436b,
            &[
                0x66, 0xf0, 0xff, 0x47, 0x10, 0xf6, 0x47, 0x12, 0x01, 0x75, 0x33, 0x48, 0x8b, 0x41,
                0x60, 0x48, 0x8b, 0x51, 0x68, 0x49, 0x89, 0xd0, 0x49, 0x29, 0xc0, 0x49, 0xc1, 0xf8,
                0x02, 0x49, 0xb9, 0xab, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0x4d, 0x0f, 0xaf,
                0xc8, 0x49, 0xc1, 0xe9, 0x20, 0x0f, 0x85, 0xad, 0, 0, 0, 0x48, 0x39, 0xc2, 0x0f,
                0x84, 0x9a, 0, 0, 0,
            ],
        ),
        (
            0x4b043a9,
            &[
                0x80, 0xb9, 0x5c, 0x02, 0, 0, 0x01, 0x75, 0x40, 0x48, 0x8b, 0xb9, 0xd8, 0x01, 0, 0,
                0x48, 0x85, 0xff, 0x0f, 0x84, 0x81, 0, 0, 0,
            ],
        ),
        (
            0x4b043e7,
            &[
                0x66, 0xf0, 0xff, 0x47, 0x10, 0xf6, 0x47, 0x12, 0x01, 0x74, 0x51, 0x85, 0xf6, 0x74,
                0x49, 0x48, 0x8b, 0x71, 0x08, 0x48, 0x85, 0xf6, 0x74, 0x30,
            ],
        ),
        (
            0x4b04424,
            &[
                0x66, 0xf0, 0xff, 0x46, 0x10, 0xf6, 0x46, 0x12, 0x01, 0x75, 0x10, 0x48, 0x8b, 0x81,
                0xb0, 0, 0, 0, 0x48, 0x3b, 0x81, 0xa8, 0, 0, 0, 0x74, 0x04, 0xb0, 0x01, 0xeb, 0x02,
                0x31, 0xc0,
            ],
        ),
        (
            0x836b9c,
            &[
                0x48, 0x8d, 0x05, 0xbd, 0xe8, 0xf3, 0x0d, 0x48, 0x89, 0x01, 0x48, 0x8b, 0x59, 0x10,
                0x48, 0x85, 0xdb, 0x74, 0x2b, 0xf0, 0xff, 0x4b, 0x08,
            ],
        ),
        (
            0x4afd526,
            &[
                0x48, 0x8d, 0x1d, 0x33, 0x7f, 0xc7, 0x09, 0x48, 0x89, 0x19, 0x48, 0xc7, 0x41, 0x18,
                0, 0, 0, 0,
            ],
        ),
        (
            0x4afd5d5,
            &[
                0x48, 0x8b, 0x86, 0xc8, 0x01, 0, 0, 0x48, 0x85, 0xc0, 0x74, 0x59, 0xf0, 0xff, 0x40,
                0x08, 0x48, 0x8b, 0x86, 0xc0, 0x01, 0, 0, 0x48, 0x8b, 0x8e, 0xc8, 0x01, 0, 0, 0x49,
                0x8b, 0xbe, 0xc8, 0x01, 0, 0,
            ],
        ),
        (
            0x4afd665,
            &[
                0x48, 0x8b, 0x86, 0xe0, 0x01, 0, 0, 0x48, 0x85, 0xc0, 0x74, 0x59, 0xf0, 0xff, 0x40,
                0x08, 0x48, 0x8b, 0x86, 0xd8, 0x01, 0, 0, 0x48, 0x8b, 0x8e, 0xe0, 0x01, 0, 0, 0x49,
                0x8b, 0xbe, 0xe0, 0x01, 0, 0,
            ],
        ),
        (
            0x468fd3c,
            &[
                0x4c, 0x8b, 0x78, 0x08, 0x4d, 0x85, 0xff, 0x0f, 0x84, 0x2e, 0x05, 0, 0,
            ],
        ),
        (
            0x468fd70,
            &[
                0x66, 0xf0, 0x41, 0xff, 0x47, 0x10, 0x41, 0xf6, 0x47, 0x12, 0x01, 0x0f, 0x84, 0xf6,
                0x04, 0, 0,
            ],
        ),
    ];
    for &(rva, expected) in sites {
        let mut actual = vec![0; expected.len()];
        read(
            base.checked_add(rva)
                .ok_or("ordinary frame-resource address overflow")?,
            &mut actual,
        )?;
        if actual != expected {
            return Err(format!(
                "ordinary frame-resource profile differs at {rva:x}"
            ));
        }
    }
    if crate::world_hooks::pointer(
        base.checked_add(RESOURCE_TABLE)
            .ok_or("ordinary frame-resource table overflow")?,
    ) != base.checked_add(0x836b90)
    {
        return Err("ordinary frame-resource table differs".into());
    }
    PROFILE.store(base, Ordering::Release);
    Ok(())
}
