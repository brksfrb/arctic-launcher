//! Owned, jointly rechecked model/view/projection copies from the native draw.
//! This synchronous snapshot creates no camera, scene or GPU ownership lease.
use crate::native_scene_layout::{draw_context as context, matrix_stacks as layout};
use crate::world_hooks::{pointer, read};

const MAX_CAPACITY: usize = 4096;
const MAX_DEPTH: usize = 64;
const POINTER_END: usize = 0x0000_8000_0000_0000;
fn span(address: usize, length: usize) -> Option<()> {
    (address >= 0x10000
        && address
            .checked_add(length)
            .is_some_and(|end| end <= POINTER_END))
    .then_some(())
}
fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn overlaps(a: usize, n: usize, b: usize, m: usize) -> bool {
    match (a.checked_add(n), b.checked_add(m)) {
        (Some(end), Some(other)) => a < other && b < end,
        _ => true,
    }
}
struct Stack {
    address: usize,
    header: [u8; layout::HEADER_BYTES],
    map: usize,
    map_bytes: usize,
    slot: usize,
    matrix: usize,
    bytes: [u8; layout::MATRIX_BYTES],
    owned: [f32; 16],
}
impl Stack {
    fn copy(address: usize) -> Option<Self> {
        span(address, layout::HEADER_BYTES)?;
        let mut header = [0u8; layout::HEADER_BYTES];
        read(address, &mut header).ok()?;
        let map = word(&header, layout::MAP)?;
        let capacity = word(&header, layout::CAPACITY)?;
        let offset = word(&header, layout::OFFSET)?;
        let count = word(&header, layout::COUNT)?;
        if !(8..=MAX_CAPACITY).contains(&capacity)
            || !capacity.is_power_of_two()
            || count == 0
            || count > MAX_DEPTH
            || count > capacity
            || !map.is_multiple_of(layout::SLOT_BYTES)
        {
            return None;
        }
        let map_bytes = capacity.checked_mul(layout::SLOT_BYTES)?;
        span(map, map_bytes)?;
        // Native add/dec is machine-width arithmetic. Its final mask selects
        // one 64-byte matrix block, including a wrapped back-of-deque index.
        let index = offset.wrapping_add(count).wrapping_sub(1) & (capacity - 1);
        let slot = map.checked_add(index.checked_mul(layout::SLOT_BYTES)?)?;
        let matrix = pointer(slot)?;
        span(matrix, layout::MATRIX_BYTES)?;
        let mut bytes = [0u8; layout::MATRIX_BYTES];
        read(matrix, &mut bytes).ok()?;
        let owned: [f32; 16] = std::array::from_fn(|i| {
            f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().expect("four bytes"))
        });
        if owned
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 32_000_000.)
        {
            return None;
        }
        Some(Self {
            address,
            header,
            map,
            map_bytes,
            slot,
            matrix,
            bytes,
            owned,
        })
    }
    fn current(&self) -> Option<()> {
        let mut header = [0u8; layout::HEADER_BYTES];
        read(self.address, &mut header).ok()?;
        if header != self.header || pointer(self.slot)? != self.matrix {
            return None;
        }
        let mut bytes = [0u8; layout::MATRIX_BYTES];
        read(self.matrix, &mut bytes).ok()?;
        if bytes != self.bytes {
            return None;
        }
        read(self.address, &mut header).ok()?;
        (header == self.header && pointer(self.slot)? == self.matrix).then_some(())
    }
}
pub(super) struct Matrices {
    pub(super) model: [f32; 16],
    pub(super) view: [f32; 16],
    pub(super) projection: [f32; 16],
}
pub(super) fn copy(draw_context: usize) -> Option<Matrices> {
    let screen_address = draw_context.checked_add(context::SCREEN)?;
    let screen = pointer(screen_address)?;
    let camera_address = screen.checked_add(context::CAMERA_STACKS)?;
    let camera = pointer(camera_address)?;
    let model = Stack::copy(camera.checked_add(layout::MODEL)?)?;
    let view = Stack::copy(camera.checked_add(layout::VIEW)?)?;
    let projection = Stack::copy(camera.checked_add(layout::PROJECTION)?)?;
    let stacks = [&model, &view, &projection];
    // A std::deque's map and matrix blocks are allocations, not aliases of
    // the stack headers or their pointer maps. Reject such structural overlap.
    for stack in stacks {
        for other in stacks {
            if overlaps(
                stack.map,
                stack.map_bytes,
                other.address,
                layout::HEADER_BYTES,
            ) || overlaps(
                stack.matrix,
                layout::MATRIX_BYTES,
                other.address,
                layout::HEADER_BYTES,
            ) || overlaps(
                stack.matrix,
                layout::MATRIX_BYTES,
                other.map,
                other.map_bytes,
            ) {
                return None;
            }
        }
    }
    if pointer(screen_address)? != screen || pointer(camera_address)? != camera {
        return None;
    }
    // Keep all three original snapshots alive through the complete recheck;
    // validating each independently before reading the next is insufficient.
    for stack in stacks {
        stack.current()?;
    }
    if pointer(screen_address)? != screen || pointer(camera_address)? != camera {
        return None;
    }
    Some(Matrices {
        model: model.owned,
        view: view.owned,
        projection: projection.owned,
    })
}
pub(super) fn stack_top(address: usize) -> Option<[f32; 16]> {
    let stack = Stack::copy(address)?;
    stack.current()?;
    Some(stack.owned)
}
pub(super) fn status() -> serde_json::Value {
    serde_json::json!({"sourceImplemented":true,"layout":layout::LAYOUT,
        "maximumCapacity":MAX_CAPACITY,"maximumDepth":MAX_DEPTH,"jointMatrixRecheck":true,
        "matrixBitsRechecked":true,"retainsBorrowedAddresses":false,"runtimeVerified":false})
}
