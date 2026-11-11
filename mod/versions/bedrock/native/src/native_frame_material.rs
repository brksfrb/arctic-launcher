//! Borrowed MaterialPtr shared pairs from the selected view's actual layers.
//! Positive controls do not prove a resolved shader, retained lease or GPU draw.
use crate::native_scene_layout::material as layout;
use crate::native_scene_layout::skinned_description as desc;
use crate::{native_frame_chunks::Copies, world_hooks::read};
#[path = "native_material_owner.rs"]
mod owner;
#[path = "native_skinned_material_profile.rs"]
mod skinned_profile;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    sync::atomic::{AtomicUsize, Ordering},
};

pub(crate) const MAX_MATERIALS: usize = 16_384;
pub(crate) const MAX_SHADER_PATH_BYTES: usize = 512;
// Bounds on retained metadata copies, separate from atomic owner acquisition
// and repeated endpoint checks while the synchronous consumer remains active.
pub(crate) const MAX_SKINNED_MATERIAL_BYTES: usize =
    16 + 8 + 8 + 96 + 3 * (MAX_SHADER_PATH_BYTES + 1) + crate::native_material_variation::MAX_BYTES;
pub(crate) const MAX_SKINNED_MATERIAL_BLOCK_READS: usize =
    7 + crate::native_material_variation::MAX_READS;
const MATERIAL_TABLE: usize = layout::VTABLE;
static PROFILE: AtomicUsize = AtomicUsize::new(0);
static SKINNED_PROFILE: AtomicUsize = AtomicUsize::new(0);

fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn canonical(address: usize) -> bool {
    (0x10000..0x0000_8000_0000_0000).contains(&address)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShaderPaths {
    pub digest: [u8; 32],
    pub stages: [bool; 3],
}
/// Only the installed concrete RenderMaterial class is admitted. Paths are
/// declaration metadata, not compiled programs, pipeline binding or a lease.
pub(crate) fn shader_paths(material: usize, copies: &mut Copies) -> Option<ShaderPaths> {
    let base = PROFILE.load(Ordering::Acquire);
    shader_paths_in(base, material, copies)
}
fn shader_paths_in(base: usize, material: usize, copies: &mut Copies) -> Option<ShaderPaths> {
    if base == 0 || crate::world_hooks::image_base() != Some(base) {
        return None;
    }
    let table = copies.block(material, 8)?;
    if word(&table, 0)? != base.checked_add(MATERIAL_TABLE)? {
        return None;
    }
    // parseShader stores vertex/fragment/geometry PathBuffer<string> headers
    // at d0/f0/110. Retain the whole adjacent cohort in the endpoint guards.
    let headers = copies.block(material.checked_add(layout::SHADER_PATHS)?, 96)?;
    let mut digest = Sha256::new();
    digest.update(b"arctic-native-material-shader-paths-v1\0");
    let mut stages = [false; 3];
    for stage in 0..3 {
        let header = headers.get(stage * 32..stage * 32 + 32)?;
        let length = word(header, 16)?;
        let capacity = word(header, 24)?;
        if length > MAX_SHADER_PATH_BYTES || capacity < length || capacity > 1_048_576 {
            return None;
        }
        let value = if capacity < 16 {
            header.get(..length.checked_add(1)?)?.to_vec()
        } else {
            copies.block(word(header, 0)?, length.checked_add(1)?)?
        };
        if value.last() != Some(&0) {
            return None;
        }
        let path = std::str::from_utf8(value.get(..length)?).ok()?;
        if path.chars().any(char::is_control) {
            return None;
        }
        stages[stage] = length != 0;
        digest.update([stage as u8]);
        digest.update((length as u32).to_le_bytes());
        digest.update(path.as_bytes());
    }
    Some(ShaderPaths {
        digest: digest.finalize().into(),
        stages,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sample {
    pub object: usize,
    pub control: usize,
    pub render_material: Option<usize>,
}
impl Sample {
    pub(crate) fn copy(
        address: usize,
        copies: &mut Copies,
        owners: &mut HashSet<usize>,
    ) -> Option<Self> {
        let base = PROFILE.load(Ordering::Acquire);
        Self::copy_in(base, address, copies, owners)
    }
    fn copy_in(
        base: usize,
        address: usize,
        copies: &mut Copies,
        owners: &mut HashSet<usize>,
    ) -> Option<Self> {
        if base == 0 || crate::world_hooks::image_base() != Some(base) {
            return None;
        }
        let bytes = copies.block(address, 16)?;
        let object = word(&bytes, 0)?;
        let control = word(&bytes, 8)?;
        if object != 0 && (!canonical(object) || control == 0) {
            return None;
        }
        if control != 0 {
            copies.owner(control, owners)?;
        }
        // Installed loader 5a5b080 reads info+40 and constructs its 0x48-byte
        // info at control+10 in a 0x58-byte allocation. Do not impose that
        // allocation relation on arbitrary aliasing shared references here.
        let render_material = if object == 0 {
            None
        } else {
            let bytes = copies.block(object.checked_add(layout::INFO_RENDER_MATERIAL)?, 8)?;
            let material = word(&bytes, 0)?;
            if material != 0 && !canonical(material) {
                return None;
            }
            Some(material)
        };
        // The unique_ptr value stays a private borrowed key. Its pointee,
        // shader state and resource lifetime are not traversed or inferred.
        Some(Self {
            object,
            control,
            render_material,
        })
    }
}

/// A cohort for one synchronous native skinned submission. Only its info object
/// is retained; nested material/resources and scene ownership remain borrowed.
/// The owner is !Send/!Sync and never enters asynchronous GPU retirement.
pub(crate) struct SkinnedMaterial {
    copies: Copies,
    paths: ShaderPaths,
    variation: Option<crate::native_material_variation::Guard>,
    info_owner: owner::Owner,
}
impl SkinnedMaterial {
    /// # Safety
    /// The native callback owns this live description and its strong MaterialPtr
    /// during acquisition. A pointer read or positive count does not prove that.
    pub(crate) unsafe fn copy(description: usize) -> Option<Self> {
        let base = SKINNED_PROFILE.load(Ordering::Acquire);
        let mut copies = Copies::default();
        let mut owners = HashSet::new();
        // The selected constructor copies its MaterialPtr pair to the family
        // member (legacy +168, current candidate +140); its consumer follows
        // that info object's +40 pointee, not another argument.
        let sample = Sample::copy_in(
            base,
            description.checked_add(desc::MATERIAL)?,
            &mut copies,
            &mut owners,
        )?;
        if sample.object == 0 || sample.control == 0 {
            return None;
        }
        // SAFETY: native callback ownership is the caller's contract. The
        // concrete info/control class is checked independently before retention.
        let info_owner = unsafe { owner::Owner::retain(base, sample.object, sample.control) }?;
        let material = sample.render_material.filter(|value| *value != 0)?;
        // Require the concrete class and readable path cohort here. Unknown
        // metadata may remain optional for terrain observation, but cannot
        // authorize this replay-input copy. No digest proves a compiled shader.
        let paths = shader_paths_in(base, material, &mut copies)?;
        // The focused binding now also checks the actual native define-set
        // copy/clone/node construction chain and this family's state fields.
        let variation = crate::native_material_variation::Guard::copy(material);
        let result = Self {
            copies,
            paths,
            variation,
            info_owner,
        };
        result.current().then_some(result)
    }
    pub(crate) fn current(&self) -> bool {
        self.info_owner.current()
            && self.copies.unchanged().is_some()
            && self
                .variation
                .as_ref()
                .is_none_or(|value| value.current() && self.copies.unchanged().is_some())
    }
    pub(crate) fn paths(&self) -> crate::native_shader_inputs::MaterialPaths {
        crate::native_shader_inputs::MaterialPaths {
            sha256: self
                .paths
                .digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            stages: self.paths.stages,
            variation_sha256: self.variation.as_ref().map(|value| value.fingerprint()),
        }
    }
}
pub(crate) fn owner_status() -> serde_json::Value {
    let mut value = owner::status();
    value["skinnedReaderInstalled"] =
        serde_json::json!(SKINNED_PROFILE.load(Ordering::Acquire) != 0);
    value["skinnedVariationReaderInstalled"] =
        serde_json::json!(SKINNED_PROFILE.load(Ordering::Acquire) != 0);
    value["terrainReaderInstalled"] = serde_json::json!(PROFILE.load(Ordering::Acquire) != 0);
    value
}
/// Run before either the description constructor or its material helper is
/// patched. The exact code catalog never selects an executable image family.
pub(crate) fn install_skinned(base: usize) -> Result<(), String> {
    skinned_profile::install(base)?;
    SKINNED_PROFILE.store(base, Ordering::Release);
    Ok(())
}

pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated frame-material image".into());
    }
    let sites: &[(usize, &[u8])] = &[
        // The same constructor is called for the camera's named materials.
        (
            0x466cc8e,
            &[
                0x48, 0x8d, 0x15, 0x73, 0xe6, 0x7c, 0x0d, 0x4c, 0x8d, 0x85, 0x20, 0x01, 0, 0, 0x4c,
                0x89, 0xf9, 0xe8, 0x5c, 0x51, 0xc8, 0,
            ],
        ),
        // MaterialPtr receives the group's returned shared pair, with control
        // at +8. The replacement's old strong/weak controls are released.
        (
            0x52f1e18,
            &[
                0x49, 0x89, 0xd1, 0x0f, 0x57, 0xf6, 0x48, 0x89, 0x4d, 0xe0, 0x0f, 0x11, 0x31, 0x48,
                0x8b, 0x02, 0x48, 0x8b, 0x40, 0x08, 0x48, 0x8d, 0x55, 0xd0, 0x4c, 0x89, 0xc9, 0xff,
                0x15, 0x6f, 0x9c, 0xe0, 0x09, 0x0f, 0x28, 0x45, 0xd0, 0x0f, 0x29, 0x75, 0xd0, 0x48,
                0x8b, 0x7d, 0xe0, 0x48, 0x8b, 0x77, 0x08, 0x0f, 0x11, 0x07, 0x48, 0x85, 0xf6, 0x74,
                0x2b, 0xf0, 0xff, 0x4e, 0x08,
            ],
        ),
        (
            0x52f1e66,
            &[
                0xf0, 0xff, 0x4e, 0x0c, 0x75, 0x10, 0x48, 0x8b, 0x06, 0x48, 0x8b, 0x40, 0x08, 0x48,
                0x89, 0xf1, 0xff, 0x15, 0x2c, 0x9c, 0xe0, 0x09,
            ],
        ),
        // The layer producer takes the map's MaterialPtr value at node+0x30
        // and stores that address, after the chunk index, in the layer record.
        (0x468f8c2, &[0x49, 0x83, 0xc5, 0x30]),
        (0x468f92f, &[0x4c, 0x89, 0x55, 0xd0, 0x4c, 0x89, 0x6d, 0xd8]),
        // Material loader gets the map's shared object, checks its mPtr at
        // +40, and addresses the HashedString at +10.
        (
            0x5a5b0ad,
            &[0x49, 0x8b, 0x46, 0x40, 0x48, 0x83, 0x78, 0x40, 0],
        ),
        (0x5a5b0bc, &[0x48, 0x8d, 0x48, 0x10]),
        // Exact in-place MaterialPtr info control class, not a generic positive
        // shared count or another aliasing shared_ptr allocation.
        (
            0x5a5b118,
            &[0x48, 0x8d, 0x05, 0xc1, 0x9e, 0xef, 0x08, 0x48, 0x89, 0x07],
        ),
        // In-place shared construction clears through control+57, including
        // the info object's unique_ptr at control+50; map stores the pair.
        (
            0x5a5b131,
            &[
                0x48, 0xc7, 0x47, 0x40, 0x0f, 0, 0, 0, 0x0f, 0x11, 0x47, 0x48,
            ],
        ),
        (
            0x5a5b19f,
            &[
                0x49, 0x89, 0x5e, 0x40, 0x49, 0x8b, 0x5e, 0x48, 0x49, 0x89, 0x7e, 0x48,
            ],
        ),
        (0x5a5acb6, &[0x48, 0x8b, 0x43, 0x40, 0x48, 0x8b, 0x40, 0x40]),
        // Skinned constructor retains the source pair's strong control and
        // copies object/control into the description at +168/+170.
        (
            0x5e9e8b5,
            &[
                0x49, 0x8b, 0x41, 0x08, 0x48, 0x85, 0xc0, 0x74, 0x04, 0xf0, 0xff, 0x40, 0x08,
            ],
        ),
        (
            0x5e9e8d0,
            &[
                0x49, 0x8b, 0x01, 0x48, 0x89, 0x81, 0x68, 0x01, 0, 0, 0x49, 0x8b, 0x41, 0x08, 0x48,
                0x89, 0x81, 0x70, 0x01, 0, 0,
            ],
        ),
        // Material helper obtains the description from its third argument,
        // then resolves exactly that description's info/backing pair.
        (
            0x5ee8ea1,
            &[0x49, 0x8b, 0, 0x0f, 0xb6, 0xb0, 0x79, 0x01, 0, 0],
        ),
        (
            0x5ee8ecd,
            &[
                0x48, 0x8b, 0x80, 0x68, 0x01, 0, 0, 0x48, 0x85, 0xc0, 0x74, 0x15, 0x48, 0x8b, 0x58,
                0x40,
            ],
        ),
        // Defines set and adjacent 16-bit state mask in the material copy.
        (
            0x5a5c575,
            &[
                0x49, 0x8d, 0xbd, 0x98, 0, 0, 0, 0x48, 0x8d, 0x96, 0x98, 0, 0, 0,
            ],
        ),
        (
            0x5a5c593,
            &[
                0x0f, 0xb7, 0x86, 0xa8, 0, 0, 0, 0x66, 0x41, 0x89, 0x85, 0xa8, 0, 0, 0,
            ],
        ),
        // Set sentinel: left/parent/right self-links, color/nil bytes 1/1.
        (
            0x118568b,
            &[
                0x48, 0x89, 0, 0x48, 0x89, 0x40, 0x08, 0x48, 0x89, 0x40, 0x10, 0x66, 0xc7, 0x40,
                0x18, 0x01, 0x01,
            ],
        ),
        (0x11856b8, &[0x48, 0x8b, 0x47, 0x08, 0x48, 0x89, 0x46, 0x08]),
        // Native define reader uses first node, then its string at +20,
        // length at +30 and capacity at +38; iterator uses +10/+8/+0 links.
        (
            0x8ca77fb,
            &[0x4c, 0x8b, 0x12, 0x4d, 0x8b, 0x3a, 0x4d, 0x39, 0xd7],
        ),
        (
            0x8ca787c,
            &[
                0x49, 0x83, 0x7f, 0x38, 0x10, 0x72, 0x0d, 0x49, 0x8b, 0x7f, 0x20,
            ],
        ),
        (0x8ca7894, &[0x49, 0x8b, 0x5f, 0x30]),
        (
            0x431b16,
            &[
                0x48, 0x89, 0x78, 0x08, 0x0f, 0xb6, 0x46, 0x18, 0x41, 0x88, 0x40, 0x18, 0x48, 0x8b,
                0x16,
            ],
        ),
        (0x8ca7a60, &[0x49, 0x8b, 0x4f, 0x10, 0x80, 0x79, 0x19, 0]),
        // Concrete material copy constructor replaces the base vptr. The
        // parseShader override selects the same three named path fields.
        (
            0x5a5cb58,
            &[
                0x48, 0x8d, 0x05, 0xf1, 0xa9, 0xe9, 0x08, 0x49, 0x89, 0x45, 0,
            ],
        ),
        (0x4b0b673, &[0x4c, 0x8d, 0x86, 0xd0, 0, 0, 0]),
        (0x4b0b69c, &[0x4c, 0x8d, 0x86, 0xf0, 0, 0, 0]),
        (0x4b0b6c5, &[0x48, 0x81, 0xc6, 0x10, 0x01, 0, 0]),
        (0xec0c298, b"vertexShader\0"),
        (0xec0c2a5, b"fragmentShader\0"),
        (0xec0c289, b"geometryShader\0"),
    ];
    for &(rva, expected) in sites {
        let mut actual = vec![0; expected.len()];
        read(
            base.checked_add(rva)
                .ok_or("frame-material address overflow")?,
            &mut actual,
        )?;
        if actual != expected {
            return Err(format!("frame-material profile differs at {rva:x}"));
        }
    }
    // Table pointers relocate with the image. Compare rebased targets rather
    // than preferred-address bytes; no virtual function is called.
    for (index, rva) in [0x4b163f0usize, 0x4b07e60, 0x76bc0, 0x4b0bf60, 0x4b0b640]
        .into_iter()
        .enumerate()
    {
        let mut bytes = [0; 8];
        read(
            base.checked_add(MATERIAL_TABLE + index * 8)
                .ok_or("material table address overflow")?,
            &mut bytes,
        )?;
        if word(&bytes, 0) != base.checked_add(rva) {
            return Err("concrete material table differs".into());
        }
    }
    for (index, rva) in [
        layout::INFO_DESTROY_RVA,
        layout::INFO_DELETE_RVA,
        layout::INFO_DELETING_DTOR_RVA,
    ]
    .into_iter()
    .enumerate()
    {
        let mut bytes = [0; 8];
        read(
            base.checked_add(layout::INFO_CONTROL_VTABLE + index * 8)
                .ok_or("material owner table overflow")?,
            &mut bytes,
        )?;
        if word(&bytes, 0) != base.checked_add(rva) {
            return Err("material info control class differs".into());
        }
    }
    PROFILE.store(base, Ordering::Release);
    Ok(())
}
