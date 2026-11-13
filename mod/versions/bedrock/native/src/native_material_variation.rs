//! Borrowed declared defines/state-mask metadata, not shader/pipeline identity.
//! Layout is independently pinned by the material copy and define consumer.
use crate::native_frame_chunks::Copies;
use crate::native_scene_layout::material_variation as layout;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub(crate) const MAX_DEFINES: usize = 128;
pub(crate) const MAX_DEFINE_BYTES: usize = 128;
pub(crate) const MAX_BYTES: usize = layout::FIELDS_BYTES
    + layout::SENTINEL_BYTES
    + MAX_DEFINES * (layout::NODE_BYTES + MAX_DEFINE_BYTES + 1);
pub(crate) const MAX_READS: usize = 2 + MAX_DEFINES * 2;
fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn node_pointer(value: usize) -> bool {
    (0x10000..0x0000_8000_0000_0000).contains(&value) && value.is_multiple_of(8)
}
fn text(header: &[u8], copies: &mut Copies) -> Option<Vec<u8>> {
    let length = word(header, layout::STRING_LENGTH)?;
    let capacity = word(header, layout::STRING_CAPACITY)?;
    if length > MAX_DEFINE_BYTES || capacity < length || capacity > 1_048_576 {
        return None;
    }
    let mut bytes = if capacity < layout::STRING_INLINE_CAPACITY {
        header.get(..length.checked_add(1)?)?.to_vec()
    } else {
        copies.block(word(header, 0)?, length.checked_add(1)?)?
    };
    if bytes.pop() != Some(0) {
        return None;
    }
    if std::str::from_utf8(&bytes)
        .ok()?
        .chars()
        .any(char::is_control)
    {
        return None;
    }
    Some(bytes)
}
enum Step {
    Visit(usize, usize),
    Emit(usize, Vec<u8>),
}
pub(crate) struct Guard {
    copies: Copies,
    digest: [u8; 32],
}
impl Guard {
    /// Called only after the concrete material class has been admitted. Keep
    /// optional-field failures separate from the path reader's own cohort.
    pub(crate) fn copy(material: usize) -> Option<Self> {
        let mut copies = Copies::default();
        // mDefines head/size followed immediately by the 16-bit mStateMask.
        let fields = copies.block(material.checked_add(layout::DEFINES)?, layout::FIELDS_BYTES)?;
        let head = word(&fields, layout::SET_HEAD)?;
        let count = word(&fields, layout::SET_COUNT)?;
        if count > MAX_DEFINES || !node_pointer(head) {
            return None;
        }
        // Never read the sentinel's uninitialized string storage.
        let sentinel = copies.block(head, layout::SENTINEL_BYTES)?;
        if sentinel[layout::COLOR] != 1 || sentinel[layout::NIL] != 1 {
            return None;
        }
        let first = word(&sentinel, layout::LEFT)?;
        let root = word(&sentinel, layout::PARENT)?;
        let last = word(&sentinel, layout::RIGHT)?;
        if ![first, root, last].into_iter().all(node_pointer) {
            return None;
        }
        let mut digest = Sha256::new();
        digest.update(b"arctic-native-material-variation-v1\0");
        let mask = layout::STATE_MASK - layout::DEFINES;
        digest.update(&fields[mask..mask + 2]);
        digest.update((count as u32).to_le_bytes());
        if count == 0 {
            if [first, root, last] != [head; 3] {
                return None;
            }
        } else {
            if [first, root, last].contains(&head) {
                return None;
            }
            let mut stack = vec![Step::Visit(root, head)];
            let mut seen = HashSet::with_capacity(count);
            let mut previous: Option<Vec<u8>> = None;
            let mut emitted = 0;
            let mut final_node = head;
            while let Some(step) = stack.pop() {
                match step {
                    Step::Visit(node, parent) => {
                        if node == head {
                            continue;
                        }
                        if !node_pointer(node) || seen.len() >= count || !seen.insert(node) {
                            return None;
                        }
                        let bytes = copies.block(node, layout::NODE_BYTES)?;
                        if bytes[layout::NIL] != 0
                            || bytes[layout::COLOR] > 1
                            || word(&bytes, layout::PARENT)? != parent
                        {
                            return None;
                        }
                        let left = word(&bytes, layout::LEFT)?;
                        let right = word(&bytes, layout::RIGHT)?;
                        if ![left, right].into_iter().all(node_pointer) {
                            return None;
                        }
                        let key = text(
                            &bytes[layout::KEY..layout::KEY + layout::STRING_BYTES],
                            &mut copies,
                        )?;
                        if stack.len().checked_add(3)? > MAX_DEFINES * 2 + 1 {
                            return None;
                        }
                        stack.push(Step::Visit(right, node));
                        stack.push(Step::Emit(node, key));
                        stack.push(Step::Visit(left, node));
                    }
                    Step::Emit(node, key) => {
                        if previous
                            .as_ref()
                            .is_some_and(|before| before.as_slice() >= key.as_slice())
                            || emitted == 0 && node != first
                        {
                            return None;
                        }
                        digest.update((key.len() as u32).to_le_bytes());
                        digest.update(&key);
                        previous = Some(key);
                        emitted += 1;
                        final_node = node;
                    }
                }
            }
            if seen.len() != count || emitted != count || final_node != last {
                return None;
            }
        }
        copies.unchanged()?;
        Some(Self {
            copies,
            digest: digest.finalize().into(),
        })
    }
    pub(crate) fn current(&self) -> bool {
        self.copies.unchanged().is_some()
    }
    pub(crate) fn fingerprint(&self) -> String {
        self.digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}
