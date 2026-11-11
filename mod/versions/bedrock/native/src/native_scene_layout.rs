//! Scene readers and replay controllers share one exact-build layout family.
//!
//! These are the existing 1.26.5203.0 bindings. Installation must match both
//! PRODUCT_VERSION and the complete executable-section hash before use. The
//! 1.26.60.5 candidates in inspection/ are migration evidence only; changing a
//! hook RVA does not migrate the scene, camera, ECS or ownership readers.

pub(crate) const PRODUCT_VERSION: &str = "1.26.5203.0";

pub(crate) mod level_tick {
    pub(crate) const VTABLE: usize = 0xe7adab0;
    pub(crate) const SLOT: usize = 0xcc0;
}

pub(crate) mod game {
    pub(crate) const VTABLE: usize = 0xe76e2d0;
    pub(crate) const RENDERER_GETTER_SLOT: usize = 0x360 / 8;
    pub(crate) const PRIMARY_CLIENTS: usize = 0x970;
    pub(crate) const RENDERER: usize = 0x1440;
}

pub(crate) mod client_map {
    pub(crate) const HEAD: usize = 0;
    pub(crate) const COUNT: usize = 8;
    pub(crate) const NODE_LEFT: usize = 0;
    pub(crate) const NODE_PARENT: usize = 8;
    pub(crate) const NODE_NIL: usize = 0x19;
    pub(crate) const NODE_KEY: usize = 0x20;
    /// ClientInstance object pointer, not the preceding interface pointer.
    /// The current-build candidate moves this field while retaining byte keys.
    pub(crate) const NODE_CLIENT: usize = 0x30;
}

pub(crate) mod client {
    pub(crate) const VTABLE: usize = 0xe9731b0;
    pub(crate) const GAME: usize = 0x1a8;
    pub(crate) const MINECRAFT: usize = 0x1b0;
    pub(crate) const LEVEL_RENDERER: usize = 0x1c0;
    pub(crate) const LEVEL_RENDERER_GETTER_SLOT: usize = 187;
    pub(crate) const LEVEL_GETTER_SLOT: usize = 168;
    pub(crate) const LOCAL_PLAYER_GETTER_SLOT: usize = 0x1f;
    pub(crate) const LOCAL_PLAYER_GETTER_RVA: usize = 0x5daed30;
    pub(crate) const CAMERA_GETTER_SLOT: usize = 43;
    pub(crate) const CAMERA_GETTER_RVA: usize = 0x5dba620;
    pub(crate) const WEAK_CAMERA_REFERENCE: usize = 0x218;
    pub(crate) const WEAK_PLAYER_REFERENCE: usize = 0x248;
}

pub(crate) mod actor {
    pub(crate) const VTABLE: usize = 0xe7caa50;
    pub(crate) const DELETING_DESTRUCTOR_SLOT: usize = 8;
    pub(crate) const CACHE: usize = 0x118;
    pub(crate) const LEVEL: usize = 0x1d8;
    pub(crate) const ANIMATION_COMPONENT: usize = 0x270;
}

pub(crate) mod model_renderer {
    pub(crate) const VTABLE: usize = 0xe942b70;
    pub(crate) const RENDER_RVA: usize = 0x558d300;
    pub(crate) const DRAW_STACK_BYTES: usize = 0x268;
    pub(crate) const OBSERVATION: &str = "helperThenDraw";
}
pub(crate) mod model_body {
    pub(crate) const LAYOUT: &str = "geometryGroups96Pairs16";
    pub(crate) const STACK_BYTES: usize = 0x58;
    pub(crate) const GEOMETRY_NAME: usize = 0x10;
    pub(crate) const GEOMETRY_HIERARCHY: usize = 0xe8;
    pub(crate) const PARAMS_PARTIAL: usize = 0x108;
}
pub(crate) mod skin_selection {
    pub(crate) const COMPONENT_PARAMS: usize = 0x88;
    pub(crate) const COMPONENT_FRAME: usize = 0x368;
    pub(crate) const COMPONENT_PAIRS: usize = 0x380;
    pub(crate) const PAIR_STRIDE: usize = 0x28;
    pub(crate) const CONTROLLER: usize = 0;
    pub(crate) const CONTROLLER_CONTROL: usize = 8;
    pub(crate) const GEOMETRY: usize = 16;
    pub(crate) const GEOMETRY_CONTROL: usize = 24;
}
pub(crate) mod hierarchy_query {
    pub(crate) const SLOT_BYTES: usize = 0x40;
    pub(crate) const VECTOR_OFFSET: usize = 0x398;
    pub(crate) const ELEMENT_BYTES: usize = 4;
}
pub(crate) mod attachment_observer {
    pub(crate) const SETUP_STACK_BYTES: usize = 0x1b8;
    pub(crate) const PARENT_REGISTER: &str = "rbx";
    pub(crate) const BONE_REGISTER: &str = "rsi";
    pub(crate) const EVAL_VIRTUAL_SLOT_BYTES: usize = 0x10;
}
pub(crate) mod constant_expression {
    pub(crate) const TABLE: usize = 0xe7d6860;
    pub(crate) const OBJECT_READ_BYTES: usize = 24;
    pub(crate) const VALUE_TAG: usize = 8;
    pub(crate) const VALUE_HASH: usize = 16;
    pub(crate) const HASH_TAG: u32 = 1;
    pub(crate) const EXPRESSION_BYTES: usize = 16;
    pub(crate) const EXPRESSION_VARIANT: usize = 8;
}
pub(crate) mod attachment {
    pub(crate) const CHILDREN: usize = 0x300;
    pub(crate) const CHILD_STRIDE: usize = 0x50;
    pub(crate) const CHILD_SHARED: usize = 0x40;
    pub(crate) const SLOTS: usize = 7;
    pub(crate) const SLOT_IN_PARAMS: usize = 0x1fc;
    pub(crate) const COMPONENT_RELOAD: usize = 0x370;
    pub(crate) const HIERARCHY_MAP: usize = 0x318;
    pub(crate) const BONE_BYTES: usize = 0xe8;
    pub(crate) const BONE_HASH: usize = 8;
    pub(crate) const BONE_MATRIX: usize = 0x38;
    pub(crate) const BONE_EXPRESSION: usize = 0xc8;
    pub(crate) const BONE_METHOD: usize = 0xe4;
    pub(crate) const BONE_OVERRIDE: usize = 0xe6;
    pub(crate) const INDEXED_METHOD: u8 = 2;
    pub(crate) const EXPRESSION_METHOD: u8 = 3;
}
pub(crate) mod encoder_submit {
    pub(crate) const STACK_BYTES: usize = 0x68;
    pub(crate) const ENCODER_FRAME: usize = 0;
    pub(crate) const ITEMS_BASE: usize = 0xc0700;
    pub(crate) const ITEMS_COUNT: usize = 0xdd17e8;
    pub(crate) const ENCODER_SUBMITTED: usize = 0x54c;
    pub(crate) const ITEM_STRIDE: usize = 0x80;
    pub(crate) const ITEM_INDICES: usize = 0x44;
    pub(crate) const ITEM_VERTICES: usize = 0x4c;
    pub(crate) const ITEM_CAPACITY: usize = 65535;
}
pub(crate) mod producer_frame {
    pub(crate) const CONTEXT_GLOBAL: usize = 0x11f2e860;
    pub(crate) const CONTEXT_ENCODER: usize = 0x13d10;
    pub(crate) const CONTEXT_SUBMIT: usize = 0x1d150c8;
    pub(crate) const CONTEXT_RENDER: usize = 0x1d150c0;
    pub(crate) const FIRST_FRAME: usize = 0x13d40;
    pub(crate) const FRAME_STRIDE: usize = 0xe809c0;
    pub(crate) const FRAME_NUMBER: usize = 0xe806d0;
}
pub(crate) mod frame_view {
    pub(crate) const OBSERVATION: &str = "setter8";
    pub(crate) const FRAME_BASE: usize = 0x300;
    pub(crate) const COUNT: usize = 256;
    pub(crate) const STRIDE: usize = 512;
    pub(crate) const RECTANGLE: usize = 16;
    pub(crate) const VIEW: usize = 32;
    pub(crate) const PROJECTION: usize = 96;
    pub(crate) const MATRIX_BYTES: usize = 64;
    pub(crate) const CONTEXT_VIEWS: usize = 0x1ec3080;
    pub(crate) const FRAME_WIDTH: usize = 0xdd1808;
    pub(crate) const FRAME_HEIGHT: usize = 0xdd180c;
}
pub(crate) mod backend_submit {
    pub(crate) const STACK_BYTES: usize = 0x1a238;
    pub(crate) const RESET_ABI: &str = "frameStereo3";
}
pub(crate) mod native_batch {
    pub(crate) const RESET_IN_STACK: usize = 0x1090;
    pub(crate) const VIEW_IN_STACK: usize = 0x1a8;
    pub(crate) const DRAW_INDEX_HANDLE: usize = 0x74;
    pub(crate) const COMMAND_LIST: usize = 0x30;
    pub(crate) const NON_INDEXED_COUNT: usize = 0x90;
    pub(crate) const NON_INDEXED_ARRAY: usize = 0x98;
    pub(crate) const NON_INDEXED_STRIDE: usize = 0x70;
    pub(crate) const NON_INDEXED_CBV: usize = 0x50;
    pub(crate) const INDEXED_COUNT: usize = 0x94;
    pub(crate) const INDEXED_ARRAY: usize = 0xa0;
    pub(crate) const INDEXED_STRIDE: usize = 0x88;
    pub(crate) const INDEXED_CBV: usize = 0x60;
}
/// Queue-relative metadata and its containing renderer offset belong to one
/// selected scene family. A retirement observation is not a scene/GPU lease.
pub(crate) mod native_queue {
    pub(crate) const CAPACITY: usize = 0x110;
    pub(crate) const RENDERER_QUEUE: usize = 0xcc9260;
    pub(crate) const QUEUE_OBJECT: usize = 0;
    pub(crate) const USED: usize = 0x2f8c028;
    pub(crate) const USED_READ: usize = 0x2f8c8a8;
    pub(crate) const USED_WRITE: usize = 0x2f8c8b0;
    pub(crate) const FREE: usize = 0x2f8c8b8;
    pub(crate) const FREE_READ: usize = 0x2f8d138;
    pub(crate) const FREE_WRITE: usize = 0x2f8d140;
    pub(crate) const FENCE_CONTEXT: usize = 0x2f8d148;
    pub(crate) const FENCE_OBJECT: usize = 0x18;
    pub(crate) const FENCE_COMPLETED: usize = 0x30;
    pub(crate) const CONTEXT_LIST: usize = 0x30;
    pub(crate) const CONTEXT_FENCE: usize = 0x2cbd0;
    pub(crate) const CONTEXT_SUBMITTED: usize = 0x2cbd8;
    pub(crate) const CONTEXT_DEFERRED: usize = 0x2cbe0;
    pub(crate) const DEFERRED_BYTES: usize = 24;
}

pub(crate) mod framebuffer {
    pub(crate) const HEADER_BYTES: usize = 32;
    pub(crate) const CONTEXT_COLORS: usize = 0x10;
    pub(crate) const CONTEXT_DEPTH: usize = 0x18;
    pub(crate) const CONTEXT_HANDLE: usize = 0x20;
    pub(crate) const CONTEXT_FLAG: usize = 0x2c;
    pub(crate) const CONTEXT_LIST: usize = 0x30;
    pub(crate) const FRAMEBUFFERS: usize = 0xc97690;
    pub(crate) const FRAMEBUFFER_STRIDE: usize = 0x68;
    pub(crate) const FRAMEBUFFER_COUNT: usize = 0x32;
    pub(crate) const FRAMEBUFFER_DEPTH: usize = 0x10;
    pub(crate) const TEXTURES: usize = 0xbde170;
    pub(crate) const STATE_CACHE: usize = 0x14458;
    pub(crate) const TEXTURE_STRIDE: usize = 0xb8;
    pub(crate) const PAGE_ENTRIES: usize = 4096;
    pub(crate) const RETIREMENT_FLAG: usize = 0x9b;
    pub(crate) const RETIREMENT_MASK: u8 = 0x40;
}

pub(crate) mod draw_context {
    pub(crate) const SCREEN: usize = 0x28;
    pub(crate) const CAMERA_STACKS: usize = 0x18;
}
pub(crate) mod matrix_stacks {
    pub(crate) const LAYOUT: &str = "msvcDequeMatrix64Block1";
    pub(crate) const VIEW: usize = 0;
    pub(crate) const MODEL: usize = 0x40;
    pub(crate) const PROJECTION: usize = 0x80;
    pub(crate) const HEADER_BYTES: usize = 40;
    pub(crate) const MAP: usize = 8;
    pub(crate) const CAPACITY: usize = 16;
    pub(crate) const OFFSET: usize = 24;
    pub(crate) const COUNT: usize = 32;
    pub(crate) const SLOT_BYTES: usize = 8;
    pub(crate) const MATRIX_BYTES: usize = 64;
}

pub(crate) mod minecraft {
    pub(crate) const VTABLE: usize = 0xe7ca5b0;
    pub(crate) const SIM_PAUSE_GETTER_SLOT: usize = 5;
    pub(crate) const SIM_PAUSE_GETTER_RVA: usize = 0x19da130;
    pub(crate) const SESSION_ACCESS_SLOT: usize = 6;
    pub(crate) const SESSION_ACCESS_RVA: usize = 0x19da150;
    pub(crate) const SESSION: usize = 0xc8;
    pub(crate) const SIM_TIMER: usize = 0xe0;
    pub(crate) const REAL_TIMER: usize = 0xe8;
}

pub(crate) mod game_session {
    pub(crate) const READY: usize = 0x28;
    pub(crate) const ACTIVE_OWNER: usize = 0x30;
    pub(crate) const ACTIVE: usize = 0;
    pub(crate) const LEVEL: usize = 0x40;
}

pub(crate) mod entity_registry {
    pub(crate) const REGISTRY: usize = 0x30;
    /// Atomic construction ID, independent of the owner's allocation address.
    pub(crate) const ID: usize = 0x160;
}

pub(crate) mod material {
    pub(crate) const VTABLE: usize = 0xe8f7550;
    pub(crate) const INFO_CONTROL_VTABLE: usize = 0xe954fe0;
    pub(crate) const INFO_DESTROY_RVA: usize = 0x5a5e650;
    pub(crate) const INFO_DELETE_RVA: usize = 0x76bd0;
    pub(crate) const INFO_DELETING_DTOR_RVA: usize = 0x280dd0;
    pub(crate) const INFO_IN_CONTROL: usize = 0x10;
    pub(crate) const INFO_RENDER_MATERIAL: usize = 0x40;
    pub(crate) const SHADER_PATHS: usize = 0xd0;
}
pub(crate) mod material_variation {
    pub(crate) const DEFINES: usize = 0x98;
    pub(crate) const SET_HEAD: usize = 0;
    pub(crate) const SET_COUNT: usize = 8;
    pub(crate) const STATE_MASK: usize = 0xa8;
    pub(crate) const FIELDS_BYTES: usize = STATE_MASK - DEFINES + 2;
    pub(crate) const SENTINEL_BYTES: usize = 32;
    pub(crate) const NODE_BYTES: usize = 64;
    pub(crate) const KEY: usize = 32;
    pub(crate) const STRING_BYTES: usize = 32;
    pub(crate) const STRING_LENGTH: usize = 16;
    pub(crate) const STRING_CAPACITY: usize = 24;
    pub(crate) const STRING_INLINE_CAPACITY: usize = 16;
    pub(crate) const LEFT: usize = 0;
    pub(crate) const PARENT: usize = 8;
    pub(crate) const RIGHT: usize = 16;
    pub(crate) const COLOR: usize = 24;
    pub(crate) const NIL: usize = 25;
}

pub(crate) mod skinned_description {
    pub(crate) const CONSTRUCTOR_FRAME_LAYOUT: &str = "legacyRbpMinus8";
    pub(crate) const MESH: usize = 0xb0;
    pub(crate) const DRAW_COUNT: usize = 0xb8;
    pub(crate) const CURRENT_BONES: usize = 0xc0;
    pub(crate) const PREVIOUS_BONES: usize = 0xd0;
    /// Distinct constructor outputs. Consumers must not infer the second
    /// member's address from the first member's size when migrating a family.
    pub(crate) const BONE_SPANS: [usize; 2] = [CURRENT_BONES, PREVIOUS_BONES];
    pub(crate) const MATERIAL: usize = 0x168;
    pub(crate) const COLOR: usize = 0x17c;
    pub(crate) const ALPHA: usize = COLOR + 12;
    pub(crate) const DITHERING: usize = 0x1fe;
}

pub(crate) mod palette_owner {
    pub(crate) const CURRENT: usize = 0x510;
    pub(crate) const PREVIOUS: usize = 0x528;
    /// The writer's last pointer argument changed from optional inline matrix
    /// to optional matrix reference independently of its seven argument slots.
    pub(crate) const PREVIOUS_MATRIX_INPUT: &str = "inlineOptionalMatrix";
}

pub(crate) mod skinned_material_consumer {
    pub(crate) const VTABLE: usize = 0xe9765c0;
    pub(crate) const INSERT_SLOT: usize = 0x358;
    pub(crate) const INPUT_LAYOUT: &str = "actorInputsPrefix184";
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmoteCacheStorage {
    Tree,
    FlatParallelVectors,
}
pub(crate) mod emote_cache {
    pub(crate) const STORAGE: super::EmoteCacheStorage = super::EmoteCacheStorage::Tree;
}

pub(crate) mod level_renderer {
    pub(crate) const VTABLE: usize = 0xe8e3bf0;
    pub(crate) const APP_VTABLE: usize = 0xe8e3cf0;
    /// Contiguous task owners, game renderer, level, options and client links.
    pub(crate) const TASK_OWNERS: usize = 0x3d8;
    pub(crate) const GAME_RENDERER: usize = 0x3e8;
    pub(crate) const LEVEL: usize = 0x3f0;
    pub(crate) const CLIENT: usize = 0x408;
    /// Shared owner: player pointer followed by its control pointer.
    pub(crate) const PLAYER_OWNER: usize = 0x468;
}

pub(crate) mod player_renderer {
    pub(crate) const VTABLE: usize = 0xe8de6b0;
    pub(crate) const OWNER_VTABLE: usize = 0xe8e4260;
    /// Adjacent float delta/last-time fields used by native preRenderUpdate.
    pub(crate) const FRAME_TIME: usize = 0x400;
    /// Contiguous level, dimension owner, dimension and renderer links.
    pub(crate) const LINKS: usize = 0x978;
    pub(crate) const DIMENSION: usize = 0x990;
    pub(crate) const LEVEL_RENDERER: usize = 0x9a0;
    /// Two consecutive vec3 values; these are distinct from the ECS camera.
    pub(crate) const PREVIOUS_ORIGIN: usize = 0x654;
    pub(crate) const ORIGIN: usize = 0x660;
}

pub(crate) mod game_renderer {
    pub(crate) const CLOCK: usize = 0x1f0;
    pub(crate) const CLOCK_SCALE: usize = 0x200;
    pub(crate) const LAST_CLOCK: usize = 0x218;
    /// Renderer dispatch counter. Its producer increments after a virtual
    /// dispatch; this alone does not certify a completed simulation/world tick.
    pub(crate) const TICK: usize = 0x220;
    pub(crate) const SCREEN_CONTEXT: usize = 0x270;
    pub(crate) const GLOBAL_CONSTANTS: usize = 0x290;
    pub(crate) const VIEW: usize = 0x388;
    pub(crate) const PROJECTION: usize = 0x408;
    pub(crate) const SKIP_WORLD_TIME: usize = 0x4c8;
    pub(crate) const WORLD_TIME: usize = 0x4d0;
}

pub(crate) mod screen {
    pub(crate) const PARTIAL: usize = 8;
    pub(crate) const GLOBAL_CONSTANTS: usize = 0x20;
    /// Pointer into GameRenderer's clock storage, not a GameRenderer pointer.
    pub(crate) const CLOCK: usize = 0xb0;
}
