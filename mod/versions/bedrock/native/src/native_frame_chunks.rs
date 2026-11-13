//! Bounded borrowed chunk/layer records from the selected native frame view.
//! Observes shared-reference liveness, never retains resources or reads GPU buffers.
use crate::native_frame_material::MAX_MATERIALS;
use crate::native_frame_material::MAX_SHADER_PATH_BYTES;
use crate::world_hooks::read;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicUsize, Ordering},
};

const MAX_CHUNKS: usize = 32_768;
const MAX_LAYERS: usize = 262_144;
const MAX_REFS: usize = MAX_CHUNKS * 6 + MAX_MATERIALS;
const MAX_BYTES: usize = 1280
    + MAX_CHUNKS * (0x48 + 128 + 25)
    + MAX_LAYERS * 0x28
    + MAX_MATERIALS * (24 + 8 + 96 + 3 * (MAX_SHADER_PATH_BYTES + 1));
const MAX_READS: usize = 4096 + MAX_CHUNKS * 12 + MAX_MATERIALS * 7;
const BLOCK: usize = 16_384;
static PROFILE: AtomicUsize = AtomicUsize::new(0);

fn word(bytes: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(
        bytes.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
    .ok()
}
fn dword(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn canonical(address: usize) -> bool {
    (0x10000..0x0000_8000_0000_0000).contains(&address)
}
fn strong(control: usize) -> Option<()> {
    if !canonical(control) {
        return None;
    }
    let mut bytes = [0; 8];
    read(control.checked_add(8)?, &mut bytes).ok()?;
    (i32::from_le_bytes(bytes[..4].try_into().ok()?) > 0
        && i32::from_le_bytes(bytes[4..].try_into().ok()?) > 0)
        .then_some(())
}
#[derive(PartialEq, Eq)]
struct Guard {
    address: usize,
    offset: usize,
    length: usize,
}
#[derive(Default, PartialEq, Eq)]
pub(crate) struct Copies {
    guards: Vec<Guard>,
    bytes: Vec<u8>,
    refs: Vec<usize>,
}
impl Copies {
    pub(crate) fn block(&mut self, address: usize, length: usize) -> Option<Vec<u8>> {
        if length == 0
            || length > BLOCK
            || self.guards.len() >= MAX_READS
            || self.bytes.len().checked_add(length)? > MAX_BYTES
            || !canonical(address)
            || address.checked_add(length)? > 0x0000_8000_0000_0000
        {
            return None;
        }
        let mut bytes = vec![0; length];
        read(address, &mut bytes).ok()?;
        self.guards.push(Guard {
            address,
            offset: self.bytes.len(),
            length,
        });
        self.bytes.extend_from_slice(&bytes);
        Some(bytes)
    }
    pub(crate) fn owner(&mut self, control: usize, seen: &mut HashSet<usize>) -> Option<()> {
        if seen.contains(&control) {
            return Some(());
        }
        if self.refs.len() >= MAX_REFS {
            return None;
        }
        strong(control)?;
        seen.insert(control);
        self.refs.push(control);
        Some(())
    }
    pub(crate) fn unchanged(&self) -> Option<()> {
        for &control in &self.refs {
            strong(control)?;
        }
        let mut bytes = [0; BLOCK];
        for guard in &self.guards {
            read(guard.address, &mut bytes[..guard.length]).ok()?;
            if bytes[..guard.length] != self.bytes[guard.offset..guard.offset + guard.length] {
                return None;
            }
        }
        for &control in &self.refs {
            strong(control)?;
        }
        Some(())
    }
}
// A reserved vector may be larger than this observer's active-record budget.
fn span(bytes: &[u8], at: usize, stride: usize, expected: usize) -> Option<usize> {
    let begin = word(bytes, at)?;
    let end = word(bytes, at.checked_add(8)?)?;
    let capacity = word(bytes, at.checked_add(16)?)?;
    if begin == 0 {
        return (end == 0 && capacity == 0 && expected == 0).then_some(0);
    }
    if ![begin, end, capacity].into_iter().all(canonical) || !begin.is_multiple_of(8) {
        return None;
    }
    let length = end.checked_sub(begin)?;
    let allocated = capacity.checked_sub(begin)?;
    if end > capacity
        || !length.is_multiple_of(stride)
        || !allocated.is_multiple_of(stride)
        || allocated / stride > 1_000_000
        || length / stride != expected
    {
        return None;
    }
    Some(begin)
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direct {
    Index {
        indices: bool,
    },
    Vertex {
        split: bool,
        vertices: bool,
        attributes: bool,
    },
}
impl Direct {
    fn copy(object: usize, vertex: bool, copies: &mut Copies) -> Option<Self> {
        if vertex {
            // The native queue producer tests these three bytes before it
            // accepts a direct mesh. Intermediate optional storage is guarded,
            // but no ServerResourcePointer or buffer payload is interpreted.
            let bytes = copies.block(object.checked_add(0x48)?, 0x41)?;
            let [split, vertices, attributes] = [bytes[0], bytes[0x20], bytes[0x40]];
            if [split, vertices, attributes]
                .into_iter()
                .any(|value| value > 1)
            {
                return None;
            }
            Some(Self::Vertex {
                split: split != 0,
                vertices: vertices != 0,
                attributes: attributes != 0,
            })
        } else {
            let available = copies.block(object.checked_add(0x60)?, 1)?[0];
            if available > 1 {
                return None;
            }
            Some(Self::Index {
                indices: available != 0,
            })
        }
    }
    fn available(self) -> bool {
        match self {
            Self::Index { indices } => indices,
            Self::Vertex {
                split,
                vertices,
                attributes,
            } => vertices && (!split || attributes),
        }
    }
}
#[derive(PartialEq, Eq)]
struct Resource {
    kind: u8,
    object: usize,
    control: usize,
    direct: Option<Direct>,
    ordinary: Option<crate::native_frame_ordinary::Sample>,
}
#[derive(Default)]
struct Resources {
    owners: HashSet<usize>,
    client_owners: HashSet<usize>,
    direct: HashMap<(bool, usize), Direct>,
    ordinary: HashMap<(bool, usize), crate::native_frame_ordinary::Sample>,
    materials: HashMap<usize, crate::native_frame_material::Sample>,
    material_owners: HashSet<usize>,
    shader_paths: HashMap<usize, Option<crate::native_frame_material::ShaderPaths>>,
}
impl Resource {
    fn copy(
        bytes: &[u8],
        at: usize,
        vertex: bool,
        copies: &mut Copies,
        resources: &mut Resources,
    ) -> Option<Self> {
        let kind = *bytes.get(at.checked_add(16)?)?;
        // monostate does not initialize its union storage. Never interpret it.
        if kind == 0 {
            return Some(Self {
                kind,
                object: 0,
                control: 0,
                direct: None,
                ordinary: None,
            });
        }
        if kind > 2 {
            return None;
        }
        let object = word(bytes, at)?;
        let control = word(bytes, at.checked_add(8)?)?;
        if object != 0 && (!canonical(object) || control == 0) {
            return None;
        }
        if control != 0 {
            copies.owner(control, &mut resources.owners)?;
        }
        let direct = if kind == 2 && object != 0 {
            let key = (vertex, object);
            let value = if let Some(value) = resources.direct.get(&key) {
                *value
            } else {
                let value = Direct::copy(object, vertex, copies)?;
                resources.direct.insert(key, value);
                value
            };
            Some(value)
        } else {
            None
        };
        let ordinary = if kind == 1 && object != 0 {
            let key = (vertex, object);
            let value = if let Some(value) = resources.ordinary.get(&key) {
                *value
            } else {
                let value = crate::native_frame_ordinary::Sample::copy(
                    object,
                    vertex,
                    copies,
                    &mut resources.client_owners,
                )?;
                resources.ordinary.insert(key, value);
                value
            };
            Some(value)
        } else {
            None
        };
        // Shared pointers may alias; object need not equal control+0x10.
        Some(Self {
            kind,
            object,
            control,
            direct,
            ordinary,
        })
    }
    fn availability(&self) -> usize {
        // Empty, unavailable, available, using the selected native predicate.
        if self.object == 0 {
            0
        } else if self.direct.is_some_and(Direct::available)
            || self.ordinary.is_some_and(|sample| sample.available)
        {
            2
        } else {
            1
        }
    }
}
#[derive(PartialEq, Eq)]
struct Chunk {
    position: [i32; 3],
    ready_time: u64,
    indices: Resource,
    mesh: Resource,
}
#[derive(Default, PartialEq, Eq)]
struct Counts {
    index_variants: [usize; 3],
    mesh_variants: [usize; 3],
    lod: [usize; 2],
    direct_indices: [usize; 3],
    direct_vertices: [usize; 3],
    direct_resources: usize,
    split_streams: usize,
    ordinary_indices: [usize; 3],
    ordinary_meshes: [usize; 3],
    ordinary_resources: usize,
    cpu_positions: usize,
    cpu_indices: usize,
    attributes: usize,
    client_refs: usize,
    client_owners: usize,
    shared_owners: usize,
    shared_refs: usize,
    unique_positions: usize,
    referenced: usize,
    masked: usize,
    fallback: usize,
    potential: usize,
    missing_mesh: usize,
    missing_material: usize,
    unavailable_direct_mesh: usize,
    unavailable_direct_indices: usize,
    unavailable_ordinary_mesh: usize,
    unavailable_ordinary_indices: usize,
    null_materials: usize,
    material_entries: usize,
    empty_materials: usize,
    live_materials: usize,
    owned_null_materials: usize,
    material_refs: usize,
    unique_materials: usize,
    material_infos: usize,
    material_owners: usize,
    missing_material_info: usize,
    render_materials: usize,
    null_render_materials: usize,
    unique_render_materials: usize,
    missing_render_material: usize,
    shader_paths_known: usize,
    shader_paths_unknown: usize,
    shader_stages: [usize; 3],
    shader_path_set_hash: Option<String>,
}
#[derive(PartialEq, Eq)]
pub(crate) struct Sample {
    chunks: Vec<Chunk>,
    layers: [usize; 51],
    counts: Counts,
    copies: Copies,
}
impl Sample {
    pub(crate) fn copy(
        collection: usize,
        expected_chunks: usize,
        layers: [usize; 51],
    ) -> Option<Self> {
        let base = PROFILE.load(Ordering::Acquire);
        if base == 0
            || crate::world_hooks::image_base() != Some(base)
            || expected_chunks > MAX_CHUNKS
        {
            return None;
        }
        let total = layers
            .iter()
            .try_fold(0usize, |sum, count| sum.checked_add(*count))?;
        if total > MAX_LAYERS {
            return None;
        }
        let mut copies = Copies::default();
        let header = copies.block(collection, 1280)?;
        let begin = span(&header, 0x18, 0x48, expected_chunks)?;
        let mut layer_begins = [0; 51];
        for (index, address) in layer_begins.iter_mut().enumerate() {
            *address = span(&header, 0x30 + index * 24, 0x28, layers[index])?;
        }
        let mut counts = Counts::default();
        let mut resources = Resources::default();
        let mut positions = HashSet::new();
        let mut chunks = Vec::with_capacity(expected_chunks);
        for first in (0..expected_chunks).step_by(64) {
            let count = 64.min(expected_chunks - first);
            let bytes = copies.block(begin.checked_add(first.checked_mul(0x48)?)?, count * 0x48)?;
            for entry in bytes.chunks_exact(0x48) {
                let position = [
                    dword(entry, 0)? as i32,
                    dword(entry, 4)? as i32,
                    dword(entry, 8)? as i32,
                ];
                let ready_time = u64::from_le_bytes(entry[0x10..0x18].try_into().ok()?);
                if !f64::from_bits(ready_time).is_finite() {
                    return None;
                }
                let indices = Resource::copy(entry, 0x18, false, &mut copies, &mut resources)?;
                let mesh = Resource::copy(entry, 0x30, true, &mut copies, &mut resources)?;
                counts.index_variants[usize::from(indices.kind)] += 1;
                counts.mesh_variants[usize::from(mesh.kind)] += 1;
                counts.shared_refs +=
                    usize::from(indices.control != 0) + usize::from(mesh.control != 0);
                if indices.kind == 2 {
                    counts.direct_indices[indices.availability()] += 1;
                }
                if mesh.kind == 2 {
                    counts.direct_vertices[mesh.availability()] += 1;
                    counts.split_streams += usize::from(matches!(
                        mesh.direct,
                        Some(Direct::Vertex { split: true, .. })
                    ));
                }
                if indices.kind == 1 {
                    counts.ordinary_indices[indices.availability()] += 1;
                }
                if mesh.kind == 1 {
                    counts.ordinary_meshes[mesh.availability()] += 1;
                }
                for sample in [indices.ordinary, mesh.ordinary].into_iter().flatten() {
                    counts.client_refs += sample.client_references;
                    counts.cpu_positions += usize::from(sample.cpu_positions);
                    counts.cpu_indices += usize::from(sample.cpu_indices);
                    counts.attributes += usize::from(sample.attributes);
                }
                positions.insert(position);
                chunks.push(Chunk {
                    position,
                    ready_time,
                    indices,
                    mesh,
                });
            }
        }
        counts.unique_positions = positions.len();
        counts.direct_resources = resources.direct.len();
        counts.ordinary_resources = resources.ordinary.len();
        counts.shared_owners = resources.owners.len();
        counts.client_owners = resources.client_owners.len();
        let mut referenced = vec![false; expected_chunks];
        let mut material_infos = HashSet::new();
        let mut render_materials = HashSet::new();
        for (queue, &address) in layer_begins.iter().enumerate() {
            for first in (0..layers[queue]).step_by(128) {
                let count = 128.min(layers[queue] - first);
                let bytes =
                    copies.block(address.checked_add(first.checked_mul(0x28)?)?, count * 0x28)?;
                for entry in bytes.chunks_exact(0x28) {
                    let index = word(entry, 0)?;
                    let chunk = chunks.get(index)?;
                    referenced[index] = true;
                    let material = word(entry, 8)?;
                    if material != 0 && !canonical(material) {
                        return None;
                    }
                    let material_reference = if material == 0 {
                        counts.null_materials += 1;
                        None
                    } else {
                        let sample = if let Some(sample) = resources.materials.get(&material) {
                            *sample
                        } else {
                            if resources.materials.len() >= MAX_MATERIALS {
                                return None;
                            }
                            let sample = crate::native_frame_material::Sample::copy(
                                material,
                                &mut copies,
                                &mut resources.material_owners,
                            )?;
                            resources.materials.insert(material, sample);
                            sample
                        };
                        counts.material_entries += 1;
                        counts.material_refs += usize::from(sample.control != 0);
                        if sample.object == 0 {
                            counts.empty_materials += 1;
                            counts.owned_null_materials += usize::from(sample.control != 0);
                        } else {
                            counts.live_materials += 1;
                            material_infos.insert(sample.object);
                            if let Some(material) =
                                sample.render_material.filter(|material| *material != 0)
                            {
                                counts.render_materials += 1;
                                render_materials.insert(material);
                                resources.shader_paths.entry(material).or_insert_with(|| {
                                    crate::native_frame_material::shader_paths(
                                        material,
                                        &mut copies,
                                    )
                                });
                            } else {
                                counts.null_render_materials += 1;
                            }
                        }
                        Some(sample)
                    };
                    let sorted_start = dword(entry, 0x10)?;
                    let sorted_count = dword(entry, 0x14)?;
                    let unsorted_start = dword(entry, 0x18)?;
                    let unsorted_count = dword(entry, 0x1c)?;
                    sorted_start.checked_add(sorted_count)?;
                    unsorted_start.checked_add(unsorted_count)?;
                    let mask = dword(entry, 0x20)?;
                    let fallback = entry[0x24];
                    let lod = usize::from(entry[0x25]);
                    if fallback > 1 || lod >= counts.lod.len() {
                        return None;
                    }
                    counts.lod[lod] += 1;
                    counts.masked += usize::from(mask != 0);
                    counts.fallback += usize::from(fallback != 0);
                    // A nonzero mask is not proof that the primary camera draws this entry.
                    if mask != 0 && (sorted_count != 0 || unsorted_count != 0) {
                        counts.potential += 1;
                        counts.missing_mesh += usize::from(chunk.mesh.object == 0);
                        counts.missing_material += usize::from(material == 0);
                        counts.missing_material_info +=
                            usize::from(material_reference.is_none_or(|sample| sample.object == 0));
                        counts.missing_render_material +=
                            usize::from(material_reference.is_none_or(|sample| {
                                sample.render_material.is_none_or(|material| material == 0)
                            }));
                        counts.unavailable_direct_mesh +=
                            usize::from(chunk.mesh.kind == 2 && chunk.mesh.availability() != 2);
                        counts.unavailable_direct_indices += usize::from(
                            chunk.indices.kind == 2 && chunk.indices.availability() != 2,
                        );
                        counts.unavailable_ordinary_mesh +=
                            usize::from(chunk.mesh.kind == 1 && chunk.mesh.availability() != 2);
                        counts.unavailable_ordinary_indices += usize::from(
                            chunk.indices.kind == 1 && chunk.indices.availability() != 2,
                        );
                    }
                }
            }
        }
        counts.referenced = referenced.into_iter().filter(|value| *value).count();
        counts.unique_materials = resources.materials.len();
        counts.material_owners = resources.material_owners.len();
        counts.material_infos = material_infos.len();
        counts.unique_render_materials = render_materials.len();
        let mut path_digests = Vec::new();
        for paths in resources.shader_paths.values() {
            if let Some(paths) = paths {
                counts.shader_paths_known += 1;
                path_digests.push(paths.digest);
                for stage in 0..3 {
                    counts.shader_stages[stage] += usize::from(paths.stages[stage]);
                }
            } else {
                counts.shader_paths_unknown += 1;
            }
        }
        // Sort copied digests, retaining per-object multiplicity. Native
        // addresses and traversal order never enter this metadata fingerprint.
        path_digests.sort_unstable();
        let mut hash = Sha256::new();
        hash.update(b"arctic-native-material-shader-path-set-v1\0");
        hash.update((path_digests.len() as u32).to_le_bytes());
        for digest in path_digests {
            hash.update(digest);
        }
        counts.shader_path_set_hash = Some(format!("{:x}", hash.finalize()));
        let sample = Self {
            chunks,
            layers,
            counts,
            copies,
        };
        sample.live().then_some(sample)
    }
    pub(crate) fn live(&self) -> bool {
        let base = PROFILE.load(Ordering::Acquire);
        base != 0
            && crate::world_hooks::image_base() == Some(base)
            && self.copies.unchanged().is_some()
    }
    pub(crate) fn receipt(&self) -> Value {
        json!({"observed":true,"chunkEntries":self.chunks.len(),"layerEntries":self.layers.iter().sum::<usize>(),
            "layerQueueCounts":self.layers.as_slice(),"indexVariantCounts":self.counts.index_variants,
            "meshVariantCounts":self.counts.mesh_variants,"lodCounts":self.counts.lod,
            "directAvailabilityObserved":true,"directIndexAvailabilityCounts":self.counts.direct_indices,
            "directVertexAvailabilityCounts":self.counts.direct_vertices,"uniqueDirectResources":self.counts.direct_resources,
            "splitStreamChunkEntries":self.counts.split_streams,
            "ordinaryAvailabilityObserved":true,"ordinaryIndexAvailabilityCounts":self.counts.ordinary_indices,
            "ordinaryMeshAvailabilityCounts":self.counts.ordinary_meshes,"uniqueOrdinaryResources":self.counts.ordinary_resources,
            "ordinaryCpuPositionFallbackEntries":self.counts.cpu_positions,"ordinaryCpuIndexFallbackEntries":self.counts.cpu_indices,
            "ordinaryAttributeBufferEntries":self.counts.attributes,"clientResourceReferences":self.counts.client_refs,
            "uniqueClientResourceOwners":self.counts.client_owners,
            "sharedResourceReferences":self.counts.shared_refs,"uniqueSharedOwners":self.counts.shared_owners,
            "uniqueChunkPositions":self.counts.unique_positions,"referencedChunks":self.counts.referenced,
            "unreferencedChunks":self.chunks.len()-self.counts.referenced,"visibleMaskEntries":self.counts.masked,
            "fallbackToUnsortedEntries":self.counts.fallback,"potentialDrawEntries":self.counts.potential,
            "missingPotentialMeshEntries":self.counts.missing_mesh,"missingPotentialMaterialEntries":self.counts.missing_material,
            "materialReferencesObserved":true,"materialSharedOwnersObserved":true,
            "renderMaterialPointersObserved":true,
            "renderMaterialReferenceEntries":self.counts.render_materials,"nullRenderMaterialReferenceEntries":self.counts.null_render_materials,
            "uniqueRenderMaterialPointers":self.counts.unique_render_materials,"missingPotentialRenderMaterialEntries":self.counts.missing_render_material,
            "materialShaderPathMetadataInspected":true,"materialShaderPathsComplete":self.counts.shader_paths_unknown==0,
            "materialShaderPathObjects":self.counts.shader_paths_known,"unknownMaterialShaderPathObjects":self.counts.shader_paths_unknown,
            "materialShaderStagePathCounts":self.counts.shader_stages,"materialShaderPathSetSha256":self.counts.shader_path_set_hash,
            "compiledMaterialProgramsObserved":false,"materialPipelineBindingVerified":false,
            "nullMaterialEntries":self.counts.null_materials,"materialReferenceEntries":self.counts.material_entries,
            "emptyMaterialReferenceEntries":self.counts.empty_materials,"liveMaterialReferenceEntries":self.counts.live_materials,
            "ownedNullMaterialReferenceEntries":self.counts.owned_null_materials,"materialSharedReferences":self.counts.material_refs,
            "uniqueMaterialReferences":self.counts.unique_materials,"uniqueMaterialInfos":self.counts.material_infos,
            "uniqueMaterialOwners":self.counts.material_owners,"missingPotentialMaterialInfoEntries":self.counts.missing_material_info,
            "unavailablePotentialDirectMeshEntries":self.counts.unavailable_direct_mesh,
            "unavailablePotentialDirectIndexEntries":self.counts.unavailable_direct_indices,
            "unavailablePotentialOrdinaryMeshEntries":self.counts.unavailable_ordinary_mesh,
            "unavailablePotentialOrdinaryIndexEntries":self.counts.unavailable_ordinary_indices,
            "rangeArithmeticValid":true,"meshIndexOwnersObserved":true,"bufferContentsObserved":false,
            "indexBufferBoundsVerified":false,"materialOwnershipObserved":false,"resourceTypesVerified":false,
            "gpuUploadVerified":false,"visibleSetVerified":false,"terrainReady":false})
    }
}
pub(crate) fn install(base: usize) -> Result<(), String> {
    if crate::world_hooks::image_base() != Some(base) {
        return Err("unvalidated frame-chunk image".into());
    }
    let sites: &[(usize, &[u8])] = &[
        (
            0x468fce7,
            &[
                0x41, 0x83, 0xf9, 0x02, 0x0f, 0x85, 0xa4, 0x05, 0, 0, 0x48, 0x85, 0xc0, 0x0f, 0x84,
                0x7d, 0x05, 0, 0, 0x80, 0x78, 0x68, 0x01, 0x0f, 0x85, 0x73, 0x05, 0, 0, 0x80, 0x78,
                0x48, 0x01, 0x75, 0x0d, 0x80, 0xb8, 0x88, 0, 0, 0, 0x01, 0x0f, 0x85, 0x60, 0x05, 0,
                0,
            ],
        ),
        (
            0x468fd99,
            &[
                0x41, 0x83, 0xf9, 0x02, 0x0f, 0x85, 0xf7, 0x04, 0, 0, 0x48, 0x85, 0xc0, 0x0f, 0x84,
                0xcb, 0x04, 0, 0, 0x80, 0x78, 0x60, 0, 0x0f, 0x84, 0xc1, 0x04, 0, 0,
            ],
        ),
        (
            0x52eb319,
            &[
                0x8b, 0x45, 0xb8, 0x41, 0x89, 0x04, 0x24, 0x8b, 0x45, 0xbc, 0x41, 0x89, 0x44, 0x24,
                0x04, 0x8b, 0x45, 0xc0, 0x41, 0x89, 0x44, 0x24, 0x08, 0xf2, 0x0f, 0x10, 0x45, 0xc8,
                0xf2, 0x41, 0x0f, 0x11, 0x44, 0x24, 0x10,
            ],
        ),
        (
            0x4703c86,
            &[
                0x0f, 0xb6, 0x41, 0x40, 0x83, 0xf8, 0x01, 0x7f, 0x06, 0x85, 0xc0, 0x75, 0x07, 0xeb,
                0x45, 0x83, 0xf8, 0x02, 0x75, 0x40, 0x48, 0x8b, 0x71, 0x38, 0x48, 0x85, 0xf6, 0x74,
                0x37, 0xf0, 0xff, 0x4e, 0x08,
            ],
        ),
        (
            0x4703cda,
            &[
                0x0f, 0xb6, 0x41, 0x28, 0x83, 0xf8, 0x01, 0x7f, 0x06, 0x85, 0xc0, 0x75, 0x07, 0xeb,
                0x43, 0x83, 0xf8, 0x02, 0x75, 0x3e, 0x48, 0x8b, 0x71, 0x20, 0x48, 0x85, 0xf6, 0x74,
                0x35, 0xf0, 0xff, 0x4e, 0x08,
            ],
        ),
        (
            0x52eb43a,
            &[
                0x48, 0x8b, 0x4d, 0xd0, 0x49, 0x89, 0x4c, 0x24, 0x18, 0x48, 0x8b, 0x4d, 0xd8, 0x49,
                0x89, 0x4c, 0x24, 0x20, 0x41, 0x88, 0x44, 0x24, 0x28,
            ],
        ),
        (
            0x52eb483,
            &[
                0x48, 0x8b, 0x45, 0xe8, 0x49, 0x89, 0x44, 0x24, 0x30, 0x48, 0x8b, 0x45, 0xf0, 0x49,
                0x89, 0x44, 0x24, 0x38, 0x41, 0x88, 0x7c, 0x24, 0x40, 0x48, 0x83, 0x46, 0x20, 0x48,
            ],
        ),
        (
            0x468f92f,
            &[
                0x4c, 0x89, 0x55, 0xd0, 0x4c, 0x89, 0x6d, 0xd8, 0x89, 0x45, 0xe0, 0x89, 0x5d, 0xe4,
                0x89, 0x4d, 0xe8, 0x89, 0x55, 0xec, 0x8b, 0x45, 0xf8, 0x89, 0x45, 0xf0, 0x44, 0x88,
                0x7d, 0xf4, 0x31, 0xc0, 0xf6, 0x45, 0xff, 0x01, 0x49, 0x0f, 0x45, 0xc3, 0x44, 0x88,
                0x45, 0xf5,
            ],
        ),
        (
            0x468f981,
            &[
                0x48, 0x8b, 0x45, 0xf0, 0x48, 0x89, 0x42, 0x20, 0x0f, 0x10, 0x45, 0xd0, 0x0f, 0x10,
                0x4d, 0xe0, 0x0f, 0x11, 0x4a, 0x10, 0x0f, 0x11, 0x02, 0x48, 0x83, 0x41, 0x08, 0x28,
            ],
        ),
    ];
    for &(rva, expected) in sites {
        let mut actual = vec![0; expected.len()];
        read(
            base.checked_add(rva)
                .ok_or("frame-chunk address overflow")?,
            &mut actual,
        )?;
        if actual != expected {
            return Err(format!("frame-chunk profile differs at {rva:x}"));
        }
    }
    crate::native_frame_ordinary::install(base)?;
    crate::native_frame_material::install(base)?;
    PROFILE.store(base, Ordering::Release);
    Ok(())
}
