//! Native Bedrock adapter. Engine-dependent features are deliberately not
//! advertised until the version-specific game bindings have been verified.

// The frame-chunk report builds one large `json!` object.
#![recursion_limit = "256"]
// The engine is built ahead of its game bindings: parts stay unused until each binding is
// verified, native hooks take the game's own argument lists, and `as_chunks` would change
// the slice types the byte parsers rely on. Matrix and palette code indexes by row on
// purpose, and its records mirror the game's own layouts, large variants included.
#![allow(
    dead_code,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::chunks_exact_to_as_chunks,
    clippy::needless_range_loop,
    clippy::large_enum_variant
)]

pub mod avatar_draw_order;
#[cfg(windows)]
pub mod avatar_draw_submit;
pub mod avatar_geometry;
#[cfg(windows)]
pub mod avatar_glint;
#[cfg(windows)]
pub mod avatar_gpu_upload;
#[cfg(windows)]
pub mod avatar_material;
pub mod bundle;
pub mod engine;
#[cfg(windows)]
mod gpu_flight;
#[cfg(windows)]
mod native_geometry_stream;

#[cfg(windows)]
pub mod capture;
#[cfg(windows)]
mod readback;

#[cfg(windows)]
mod runtime;

#[cfg(windows)]
pub mod world_hooks;

#[cfg(windows)]
mod players;

#[cfg(windows)]
mod native_dimension;

#[cfg(windows)]
mod native_game_mode;

#[cfg(windows)]
pub mod depth;

#[cfg(windows)]
pub mod depth_tracker;

#[cfg(windows)]
mod depth_pipeline;

#[cfg(windows)]
mod ecs;

#[cfg(windows)]
mod live_client;

#[cfg(windows)]
mod native_scene_layout;

#[cfg(windows)]
mod live_frame;

#[cfg(windows)]
mod render_bridge;

#[cfg(windows)]
mod native_avatar_record;
#[cfg(windows)]
mod native_batch;
#[cfg(windows)]
mod native_bound_surfaces;
#[cfg(windows)]
mod native_committed_resources;
#[cfg(windows)]
mod native_descriptor_resources;
#[cfg(windows)]
mod native_draw_assets;
#[cfg(windows)]
mod native_framebuffer;
#[cfg(windows)]
mod native_framebuffer_resources;
#[cfg(windows)]
mod native_graphics_observer;
#[cfg(windows)]
mod native_graphics_state;
#[cfg(windows)]
mod native_surface_views;

#[cfg(windows)]
mod animation_pose;

#[cfg(windows)]
mod native_appearance;
#[cfg(windows)]
mod native_camera;
#[cfg(windows)]
mod native_camera_control;
#[cfg(windows)]
mod native_capes;
#[cfg(windows)]
mod native_clock;
#[cfg(windows)]
mod native_elytra;
#[cfg(windows)]
mod native_render_clock;

#[cfg(windows)]
mod native_shader_clock;

#[cfg(windows)]
mod native_presentation_clock;

#[cfg(windows)]
mod native_flipbook;

#[cfg(windows)]
mod native_shader_inputs;

#[cfg(windows)]
mod native_shader_asset;

#[cfg(windows)]
mod native_shader_consumer;

#[cfg(windows)]
mod native_gpu_constants;

#[cfg(windows)]
mod native_shader_layout;

#[cfg(windows)]
mod native_scene_target;

#[cfg(windows)]
mod native_camera_time;
#[cfg(windows)]
mod native_fog;

#[cfg(windows)]
mod native_skins;

#[cfg(windows)]
mod native_equipment;

#[cfg(windows)]
mod native_emote_pose;

#[cfg(windows)]
mod native_emote_palette;

#[cfg(windows)]
mod native_emote_space;

#[cfg(windows)]
mod native_attachment_samples;

#[cfg(windows)]
mod native_attachment_replay;

#[cfg(windows)]
mod native_emote_upload;

#[cfg(windows)]
mod native_emote_history;

#[cfg(windows)]
mod native_emote_cache;

#[cfg(windows)]
mod native_emote_cache_history;

#[cfg(windows)]
mod native_palette_binding;

#[cfg(windows)]
mod native_palette_writer;

#[cfg(windows)]
mod native_skin_selection;

#[cfg(windows)]
mod native_attachment_binding;

#[cfg(windows)]
mod native_molang_constant;

#[cfg(windows)]
mod native_expression_history;

#[cfg(windows)]
mod native_expression_binding;

#[cfg(windows)]
mod native_cache_binding;

#[cfg(windows)]
mod native_world_work;

#[cfg(windows)]
mod native_task_activity;

#[cfg(windows)]
mod native_network_work;

#[cfg(windows)]
mod native_render_work;

#[cfg(windows)]
mod native_chunk_work;

#[cfg(windows)]
mod native_shared_chunk_work;

#[cfg(windows)]
mod native_frame_input;

#[cfg(windows)]
mod native_frame_chunks;

#[cfg(windows)]
mod native_frame_ordinary;

#[cfg(windows)]
mod native_frame_material;

#[cfg(windows)]
mod native_material_variation;

#[cfg(windows)]
mod native_cape_movement;

#[cfg(windows)]
mod native_cape_phase;

#[cfg(windows)]
mod native_resource_packs;

#[cfg(windows)]
mod native_input;

#[cfg(windows)]
mod native_packet;

#[cfg(windows)]
mod native_body;
#[cfg(windows)]
mod native_skinned_desc;
#[cfg(windows)]
mod native_submit;
#[cfg(windows)]
mod native_transform;

#[cfg(windows)]
pub mod native_hash;

#[cfg(windows)]
pub mod probe_isolation;

#[cfg(windows)]
#[unsafe(no_mangle)]
/// Completed DX12 adapter callbacks for diagnostics. TEST presents are excluded;
/// this counter is not a deterministic replay acknowledgement.
pub extern "C" fn arctic_bedrock_renderer_frames_v1() -> u64 {
    hudhook::hooks::dx12::diagnostics().frames
}

#[cfg(windows)]
#[unsafe(no_mangle)]
/// Windows DLL entry point.
///
/// # Safety
/// Called by the Windows loader with this DLL's valid module handle. The handle
/// must remain valid while the initialization worker and render hooks are live.
pub unsafe extern "system" fn DllMain(
    module: hudhook::windows::Win32::Foundation::HINSTANCE,
    reason: u32,
    _: *mut core::ffi::c_void,
) -> i32 {
    if reason == 1 {
        // The JVM, filesystem and graphics hooks must never run under loader lock.
        let address = module.0 as usize;
        std::thread::spawn(move || runtime::start(address));
    }
    1
}
