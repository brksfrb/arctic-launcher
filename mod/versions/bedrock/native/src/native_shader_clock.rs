//! Read-only shader-clock observations for the actual primary world pass.
//! This samples CPU results/storage, not shader upload, GPU work or video parity.
use crate::native_scene_layout as scene;
use crate::world_hooks::{CameraSample, FmodBinding, RendererClockIdentity, floats, pointer, read};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy)]
struct Remainder {
    input: f64,
    native: f64,
    effective: f64,
    overridden: bool,
}
impl Remainder {
    fn value(self) -> Value {
        json!({"input":self.input,"nativeResult":self.native,"effectiveResult":self.effective,
            "arcticOverride":self.overridden})
    }
}
#[derive(Clone)]
struct Frame {
    id: u64,
    owner: RendererClockIdentity,
    output: usize,
    header: [u8; 0x50],
    binding: FmodBinding,
    domain: crate::native_presentation_clock::Domain,
    presentation_before: f64,
    phase_before: u64,
    next: u8,
    total: Option<Remainder>,
    delta: Option<Remainder>,
    world: Option<Remainder>,
}
thread_local! { static FRAME: RefCell<Option<Frame>> = const { RefCell::new(None) }; }
static NEXT: AtomicU64 = AtomicU64::new(0);

fn phase(renderer: usize) -> Option<f64> {
    let mut bytes = [0u8; 8];
    read(
        renderer.checked_add(scene::game_renderer::WORLD_TIME)?,
        &mut bytes,
    )
    .ok()?;
    let value = f64::from_le_bytes(bytes);
    (value.is_finite() && (0. ..3600.).contains(&value)).then_some(value)
}
pub(crate) fn clear() {
    FRAME.with(|cell| {
        if let Ok(mut frame) = cell.try_borrow_mut() {
            *frame = None;
        }
    });
}

pub(crate) fn begin(renderer: usize, output: usize, binding: FmodBinding) -> Option<()> {
    clear();
    let owner = crate::world_hooks::renderer_clock_identity(renderer)?;
    let presentation = crate::native_presentation_clock::sample()?;
    let domain = presentation.domain();
    let presentation_before = presentation.seconds();
    if pointer(output.checked_add(scene::screen::CLOCK)?)?
        != renderer.checked_add(scene::game_renderer::CLOCK)?
    {
        return None;
    }
    let mut header = [0u8; 0x50];
    read(output, &mut header).ok()?;
    if u32::from_le_bytes(header[8..12].try_into().ok()?) != owner.partial_bits {
        return None;
    }
    let phase_before = phase(renderer)?.to_bits();
    if crate::world_hooks::renderer_clock_identity(renderer)? != owner {
        return None;
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed).checked_add(1)?;
    FRAME.with(|cell| {
        *cell.try_borrow_mut().ok()? = Some(Frame {
            id,
            owner,
            output,
            header,
            binding,
            domain,
            presentation_before,
            phase_before,
            next: 0,
            total: None,
            delta: None,
            world: None,
        });
        Some(())
    })
}

/// Record the value actually returned by Arctic's import hook, including the
/// native result when there is no control lease. No engine code is invoked here.
pub(crate) fn sample(
    caller: usize,
    input: f64,
    divisor: f64,
    native: f64,
    effective: f64,
    overridden: bool,
) {
    FRAME.with(|cell| {
        let Ok(mut slot) = cell.try_borrow_mut() else {
            return;
        };
        let Some(frame) = slot.as_mut() else {
            return;
        };
        let role = if caller == frame.binding.total_return_rva {
            0
        } else if caller == frame.binding.delta_return_rva {
            1
        } else if caller == frame.binding.world_return_rva {
            2
        } else {
            return;
        };
        if role != frame.next
            || divisor.to_bits() != 3600f64.to_bits()
            || !input.is_finite()
            || !(0. ..=1_000_000_000.).contains(&input)
            || [native, effective]
                .iter()
                .any(|value| !value.is_finite() || !(0. ..3600.).contains(value))
        {
            *slot = None;
            return;
        }
        let value = Remainder {
            input,
            native,
            effective,
            overridden,
        };
        if role == 0 {
            let after = crate::native_presentation_clock::sample();
            if !after.as_ref().is_some_and(|after| {
                after.domain() == frame.domain
                    && input >= frame.presentation_before
                    && input <= after.seconds()
            }) {
                *slot = None;
                return;
            }
        }
        match role {
            0 => frame.total = Some(value),
            1 => frame.delta = Some(value),
            _ => frame.world = Some(value),
        }
        frame.next += 1;
    });
}

pub(crate) fn snapshot(
    level_renderer: usize,
    screen: usize,
    camera: &CameraSample,
    world_sequence: u64,
    world_scope: u64,
) -> Option<Value> {
    if world_sequence == 0 || world_scope == 0 {
        return None;
    }
    let frame = FRAME.with(|cell| cell.try_borrow().ok()?.as_ref().cloned())?;
    let owner = frame.owner;
    if owner.level_renderer != level_renderer
        || camera.epoch? != owner.epoch
        || camera.client_ticks? != owner.tick
        || camera.partial?.to_bits() != owner.partial_bits
        || camera.ticks_per_second?.to_bits() != owner.rate_bits
        || camera.client_elapsed_ms?.to_bits() != owner.elapsed_bits
        || crate::world_hooks::renderer_clock_identity(owner.game_renderer)? != owner
    {
        return None;
    }
    if screen != frame.output
        && screen
            != owner
                .game_renderer
                .checked_add(scene::game_renderer::SCREEN_CONTEXT)?
    {
        return None;
    }
    let mut header = [0u8; 0x50];
    read(screen, &mut header).ok()?;
    if header[..12] != frame.header[..12]
        || header[16..] != frame.header[16..]
        || pointer(screen.checked_add(scene::screen::CLOCK)?)?
            != owner
                .game_renderer
                .checked_add(scene::game_renderer::CLOCK)?
    {
        return None;
    }
    let total = frame.total?;
    let delta = frame.delta?;
    let mut flag = [0u8; 1];
    read(
        owner
            .game_renderer
            .checked_add(scene::game_renderer::SKIP_WORLD_TIME)?,
        &mut flag,
    )
    .ok()?;
    if flag[0] > 1 {
        return None;
    }
    let world_phase = phase(owner.game_renderer)?;
    let skipped = flag[0] == 1;
    if skipped {
        if frame.next != 2 || frame.world.is_some() || world_phase.to_bits() != frame.phase_before {
            return None;
        }
    } else if frame.next != 3 || frame.world?.effective.to_bits() != world_phase.to_bits() {
        return None;
    }
    let clock = floats::<4>(
        owner
            .game_renderer
            .checked_add(scene::game_renderer::CLOCK)?,
        1_000_000_000.,
    )?;
    if camera.render_clock?.map(f32::to_bits) != clock.map(f32::to_bits)
        || crate::world_hooks::renderer_clock_identity(owner.game_renderer)? != owner
        || crate::native_presentation_clock::domain()? != frame.domain
        || phase(owner.game_renderer)?.to_bits() != world_phase.to_bits()
    {
        return None;
    }
    let mut after = [0u8; 1];
    read(
        owner
            .game_renderer
            .checked_add(scene::game_renderer::SKIP_WORLD_TIME)?,
        &mut after,
    )
    .ok()?;
    if after != flag {
        return None;
    }
    Some(
        json!({"schema":2,"source":"native_frame_fmod","session":crate::world_hooks::native_session(owner.epoch),
        "presentationClockDomain":frame.domain.value(),
        "observation":frame.id,"worldSequence":world_sequence,"worldScope":world_scope,
        "ticks":owner.tick,"elapsedMs":f64::from_bits(owner.elapsed_bits),
        "partial":f32::from_bits(owner.partial_bits),"ticksPerSecond":f32::from_bits(owner.rate_bits),
        "total":total.value(),"delta":delta.value(),"world":frame.world.map(Remainder::value),
        "worldPhase":world_phase,"worldSkipped":skipped,"rendererClock":clock,
        "nativeWorldStoreMatched":true,"shaderUploadVerified":false,"deterministicVideoReady":false}),
    )
}
