# Native binding research

The local Windows 0.62.2 public vtable definitions now supply actual descriptor
heap/view/sampler observers, including optional extended samplers and sampler
feedback views. Existing public descriptor copies feed the same bounded pending/
overlap metadata. Restore snapshots retain observed referenced objects and check
known revisions before native insertion preparation; this does not freeze slots
or replace native allocation/phase ownership. No game/API code, builds or tests
ran. Source evidence and limits are in
inspection/native-descriptor-resources-source.json.

Root-signature tags now retain actual descriptor range/flag facts from the
existing public deserializer. Restore snapshots resolve bounded graphics/compute
table slots against their actual bound heaps, retaining the heap objects without
reading or freezing descriptor contents. This independently implemented metadata
path uses the local Windows 0.62.2 definitions; no game/API execution, build or
test ran. Scope and remaining resource/write ownership work are in
inspection/native-descriptor-spans-source.json.

The public committed-resource owner path now also retains observed buffer
allocations for restored graphics root addresses and vertex/index byte spans.
It uses only actual D3D12 results and the existing local Windows API definitions;
numeric GPU addresses are never turned into CPU or COM pointers. Snapshot owners
flow into the existing native draw-asset lifetime path. No game/API code, build
or test ran. Scope and remaining descriptor/phase requirements are recorded in
inspection/native-restore-buffer-owners-source.json.

Cached texture-creation bytes and IID/heap-property data identify a direct
committed-resource route and a separate allocator helper. The public committed
resource observer and bounded owner cache are independently implemented against
the local Windows 0.62.2 API definitions. The source retains actual API outputs,
not copied registry pointers, and carries them into matched framebuffer bindings.
No game bytes/assets are bundled or executed. Evidence and unresolved allocator,
descriptor/phase and live insertion boundaries are in
inspection/native-committed-resources-source.json; runtime parity is unverified.

Cached completion-routine bytes now connect that deferred vector to the existing
native fence/used-to-free observer: copied COM references are released before
free-ring insertion. Arctic's source adds bounded header-coherence and drain
checks without calling native routines or inferring an allocation owner. See
inspection/native-fence-retirement-source.json. No game, build or test ran.

Cached native texture teardown/override/getter bytes now identify the ordinary
retirement bypass bit and a deferred pointer-vector path. Both cached text
copies were hashed against the pinned installed build. The reader guards that
mode byte without invoking those routines or inferring a native lease. No game
code executed; evidence and remaining ownership requirements are in
inspection/native-texture-retirement-source.json.

Wing emission now independently follows cached Java 26.2 WingsLayer root
translation, ElytraModel local poses and baked child-map traversal. Completed
installed-game wing matrices remain independent model-space inputs, without a
second humanoid emote parent. Cached class bytes were decoded as data without
Java/game execution. Scope and remaining ownership/rendering boundaries are in
inspection/wing-layer-root-source.json.

Shared affine pose arithmetic now independently follows cached JOML 1.10.8
generic point/product/translation float grouping and the repository Fabric
fit branch. The archive was hashed and class bytes decoded as data only.
Original target matrix properties and native pose/normal-history equivalence
are not inferred. Reference offsets, hash and source/runtime boundaries are in
inspection/affine-pose-arithmetic-source.json.

Normal player-part emission now independently follows the cached stock Java
26.2 PartDefinition bake/ModelPart traversal and the repository Fabric emote
hide branch. Cached Guava/fastutil class bytes establish the map construction
and retained traversal; those archives were hashed without class loading.
No game assets or classes are redistributed by this traversal helper. Source
contract, hashes and remaining runtime/native boundaries are in
inspection/player-part-traversal-source.json.

Automatic direct-camera targets now use local wall-time leases and exact
consumer tokens through the existing control path. Expiry performs no engine
write and cannot stand in for actual scoped restoration. No native offset or
game asset changed; this is independently implemented state coordination.
Source contract and remaining recovery/rendering boundaries are in
inspection/replay-camera-lease-source.json. No runtime evidence was collected.

Direct export camera ownership guards extend the existing local camera command
protocol. They compare acknowledged identities at queue admission and actual
camera consumption without changing engine offsets or camera math. Export
cleanup now releases its acknowledged direct override independently of clock
cleanup. Source contract and remaining recovery/rendering boundaries are in
inspection/replay-camera-ownership-source.json; no runtime evidence was collected.

The software quad sorter independently ports the cached Java 26.2
ProjectionType/VertexSorting/MeshData key selection and centroid arithmetic,
using JOML 1.10.8's default non-fused distance grouping. Cached class bytes were
decoded as data; no Java classes or game code loaded. The operators consume
the software renderer's submitted positions without inventing native/Java
vertex-space equivalence. Reference offsets, hashes, fallback rules and
remaining boundaries are in inspection/software-projection-quad-sort-source.json.

The software skin model phase independently follows the cached JOML 1.10.8
mulAffine translation grouping and Minecraft model/group ordering. Checked
camera matrices supply the owned submit positions; installed-Java pose or
native GPU equality is not inferred. The JOML archive was hashed and class
bytes decoded as data without loading Java classes. Source contract, hashes
and remaining native/rendering boundaries are in
inspection/software-model-phase-source.json.

Owned software solid/custom phases reuse the independently implemented
SimpleFeatureRenderPhase collector. The cached Java 26.2 archive hash was
confirmed as data; class decoding reconfirmed sortOnUpload defaults/factory
calls and disconnected-QUADS consolidation. Prepared software binding reuse
uses actual image objects in a fixed raster frame, not guessed native GPU
identities. No Java classes or game code were loaded/executed. Scope and
remaining rendering boundaries are in inspection/software-simple-phase-source.json.

Replay/export clock ownership guards independently extend the existing local
control protocol. They compare observed idle or acknowledged active state at
native admission and timer consumption, without new game offsets or native
asset/renderer assumptions. No runtime evidence was collected. Contract and
remaining clock/rendering boundaries are in
inspection/replay-clock-ownership-source.json.

Indexed camera timing receipts reuse the previously decoded pinned camera
preRenderUpdate hook and renderer-clock mapping. Source code now binds exact
outputs to guarded native producer frames and acknowledged clock identities;
no new native offsets, game inspection or runtime evidence were introduced.
Contract, optional capture fence and remaining timing/rendering boundaries
are in inspection/indexed-camera-time-source.json. No code was executed.

The software wing-glint observation path independently follows the already
decoded Java shader equations and model-phase ordering. It retains actual
base clip coordinates, consumes supplied texture/fog transforms and ports the
cached fastutil comparator quicksort for this owned software phase. It adds
no redistributed artwork, live clock guess or native foreign-phase authority.
Source contract and unverified boundaries are documented in
inspection/software-wing-glint-observation-source.json. No code was executed.

ARCGEO10 independently extends the owned staging contract with explicit glint
observations, using the previously decoded shader/phase requirements below.
Native preparation retains per-model input profiles in an aligned constant
arena and preserves the joint model schedule. This adds no captured engine
ABI, game asset, live producer or insertion owner. Contract, compatibility,
remaining integration and unverified status are documented in
inspection/native-packet-glint-source.json. Source review only; no execution.

Replay glint observations preserve the explicit selected shader transform,
modulator/strength/fog and model view pose required by the already traced
Java glint shader/phase. The new archive codec is independently implemented
and assumes no wall-clock rate, native foil equivalence or camera rebase.
No recording, archive round-trip or rendering was executed. Source contract,
compatibility and remaining producer/renderer boundaries are documented in
inspection/replay-glint-input-source.json.

Joint glint scheduling follows the previously decoded cached Java model-phase
sort over the complete original submission sequence. Source code now preserves
foreign delivery boundaries and shares typed unsubmitted-list asset reservations
across owned runs. Foreign tokens/acknowledgements do not supply or prove native
draws, poses or ownership. No code/runtime comparison was executed. Scope and
remaining live/replay boundaries are in inspection/joint-glint-phase-source.json.

Phase-specific native recording independently follows cached WingsLayer and
EquipmentLayerRenderer explicit order/increment instructions, RenderType's
blend-presence query and SubmitNodeCollection routing. First wing glint uses
order 1 blended model phase 7, distinct from order 1 custom glow phase 8. The
shared owned model sorter and recording transactions add no actual engine
callsite or foreign-feature history. Evidence and remaining ownership/replay
boundaries are in inspection/native-avatar-phase-recording-source.json.

Native armor glint independently ports the cached Java 26.2 glint shader
resources and RenderPipelines/BlendFunction state declarations. The optional
artwork path reads only the user's configured verified local archive, exports
owned pixels, checks their pinned SHA-256 and uploads a dedicated descriptor.
No reference asset is redistributed here. Cached PNG palette data supplied the
RGBA hash; no Java/JNI/shader execution or game operation occurred. The source
interfaces and unimplemented selection/phase/ownership/recording boundaries
are documented in inspection/native-armor-glint-source.json.

The overlay varying path independently follows cached Java 26.2 entity.vsh
and entity.fsh shader resources: the selected texel is emitted in the vertex
stage and read through a smooth fragment input before lightmap/fog. The HLSL
and software raster source now preserve this stage boundary. No shader was
compiled or executed; selected-texel input still requires its authoritative
producer. See inspection/entity-overlay-varying-source.json.

The owned avatar simple-phase collector independently ports cached Java 26.2
SimpleFeatureRenderPhase/FeatureSubmits and SubmitNodeStorage lifecycle, using
standard Java HashMap with the port's own immutable render-type reference keys.
Fabric CosmeticsLayer supplies the base/glow order call sites. It does not
capture or invent Java game identity hashes or Bedrock world ownership. The
ARCGEO09 contract and remaining scope boundaries are documented in
inspection/java-simple-phase-source.json. No classes were loaded or executed.

Model-submit boundaries and the translucent phase sorter derive from cached
Java 26.2 ModelFeatureRenderer.Submit, TranslucentSubmit, Mth and
TranslucentFeatureRenderPhase class data. The independently written bounded
sorter follows the comparator quicksort control flow of cached fastutil 8.5.18
IntArrays, including selection-sort/median thresholds and equal-key partitions.
No Java classes or library code were loaded or executed. Source provenance and
remaining pose/phase ownership boundaries are in inspection/java-model-submit-source.json.

Supported Java render-type reference reuse derives from cached Java 26.2
RenderTypes wrappers and Util$10/Util$11 memoizers. The independent implementation
retains owned objects for the selected factory argument keys; it does not copy
Java game objects or claim their identity hashes. PreparedRenderType's separate
pipeline/target/dynamic/scissor/texture equality remains an explicit missing
input. Source and transport boundaries are in
inspection/java-render-type-reference-source.json. No classes were executed.

Normal playback-rate control derives from the cached native advanceTime routine:
its +0xc multiplier loads at 0x4732516/0x473253b and float tick calculation at
0x4732545..0x473254d establish the scoped field. This uses no SDK method body or
replacement native clock object. The original executes its wall-clock path;
the adapter restores the prior multiplier after that call. The ten-tick output
cap remains. No control command was executed during this source port; see
inspection/native-playback-rate-source.json for remaining integration boundaries.

The spectator reader independently copies ActorGameTypeComponent through
Arctic's guarded ECS traversal. The four-byte member cross-references
[the SDK declaration](https://docs.levimc.org/LeviLamina/api/d4/da5/ActorGameTypeComponent_8h_source.html),
and direct values cross-reference the pinned
[GameType enum](https://raw.githubusercontent.com/LiteLDev/LeviLamina/4ca13569fc0d1cdeb873cf6e7c48ddcc7a74e59c/src/mc/world/level/GameType.h).
Cached native consumers independently establish the component hash, packed
stride and direct/default comparisons. No SDK implementation or game virtual
getter is invoked. Default mode optionally resolves through copied level data:
cached constructors establish the actual client-level vtable and shared-data
pair, while its slot-140 getter independently establishes the scalar offset.
This uses the pinned native layout rather than an older SDK Level offset.
Unmatched or unstable inputs stay unknown. These metadata observations do not
acquire native ownership. Evidence and remaining boundaries are in
inspection/native-spectator-source.json; live behavior remains unverified.

The copied ActorDataFlagComponent invisibility field cross-references
[ActorFlags::Invisible = 5 in LeviLamina's SDK declaration](https://docs.levimc.org/LeviLamina/api/d5/d1b/ActorFlags_8h_source.html).
Components.invisible only reads the adapter's already owned flag words; no SDK
method or virtual function is invoked. Partial Java layer inputs carry this
observation without inventing spectator mode or a cape preference. Source and
remaining coverage boundaries are in inspection/java-layer-visibility-source.json.

Remote cape phase inputs derive from independently decoded cached previous-
position system registrations, typed ECS views, callback-slot dispatch and the
complete 14-byte copy leaf. Arctic's observer joins existing native player UUIDs
and records owned endpoints; it neither invokes an SDK implementation nor writes
native position fields. The unchanged native original performs its normal copy.
No project/game code was executed during this source port. Boundaries are in
`inspection/cape-previous-position-source.json`.

Pre-movement cloak input order derives from cached Java AbstractClientPlayer
and ClientAvatarState class bytes decoded without loading/executing classes.
The independently implemented native collector uses already typed and pinned
StateVector/AABB metadata, recording owned coordinates only. No Java/SDK method
body is invoked by the adapter. Scope and remaining phase/coverage boundaries
are in `inspection/cape-pre-movement-phase-source.json`.

The cape movement observer derives its callback assignment, six-pointer wrapper,
typed ECS view strides and committed position stores from cached native
instructions, independently of SDK implementations. Java's per-call walk math
comes from cached LocalPlayer/ClientAvatarState class bytes decoded as data.
No game/project classes were loaded or executed during this work. Arctic's
owned bounded accumulator neither invokes an SDK movement method nor modifies
native components. Runtime coverage, timing and remaining boundaries are in
`inspection/native-movement-accumulator-source.json`.

Declared variation metadata cross-references the pinned
[RenderMaterialBase](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/deps/core_graphics/RenderMaterialBase.h)
member declaration. Cached material/set/node-copy and define-consumer instructions
independently establish the set, state-mask and node/string layouts. Arctic's
bounded reader, digest and v6 recording format are independently implemented;
no SDK implementation, native iterator, define helper or cache initializer is
executed. Only a fingerprint is serialized. Full material/program/GPU identity
and remaining boundaries are recorded in
`inspection/material-variation-replay-source.json`.

The skinned replay material guard derives its draw-description shared-reference
offsets and consumer chain from cached constructor/helper instructions. It reuses
the independently described material-info/path metadata reader below; no SDK
method body, engine lookup, ownership operation or GPU command is invoked.
Scope and unverified lifetime/program boundaries are recorded in
`inspection/skinned-material-reference-source.json`.

The same independently derived metadata is optionally recorded in the v5 layer
color track and compared against live declared paths during native replay. It
contains a digest/stage flags only and establishes no shader asset or compiled
program identity. The format and remaining boundaries are documented in
`inspection/material-path-replay-source.json`.

Material shader-path metadata cross-references pinned
[RenderMaterial](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src-client/mc/deps/minecraft_renderer/renderer/RenderMaterial.h)
and [RenderMaterialBase](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/deps/core_graphics/RenderMaterialBase.h)
declarations. Installed constructor/table/parser instructions independently
establish the concrete class and three field offsets. Only bounded CPU string
metadata is read; paths/addresses are not serialized. No SDK implementation,
native method, asset resolution or GPU program is executed by the reader.
Evidence and the unverified binding/lifetime boundary are in
`inspection/frame-material-shader-paths-source.json`.

The guarded material-info backing-pointer extension cross-references pinned
[MaterialPtr](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/deps/minecraft_renderer/renderer/MaterialPtr.h)
and [RenderMaterialInfo](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src-client/mc/deps/core/renderer/RenderMaterialInfo.h)
declarations. Cached installed loader instructions independently establish the
field used by the read-only observer. No SDK implementation, native lookup,
ownership operation or material pointee is copied/executed. Evidence and the
unverified resource/GPU boundary are in `inspection/frame-material-pointer-source.json`.

JavaCapeLayer's equipment branches and float translation constants derive from
cached Java 26.2 CapeLayer.hasLayer/submit class bytes; translation grouping
derives from JOML 1.10.8 Matrix4f.translateGeneric. Adapter inputs retain copied
selected-layer results; native item names/occupancy are not substituted for Java
component and asset lookup. The installed-game producer is unresolved. Source
scope and unverified runtime boundaries are in
`inspection/cape-equipment-layer-source.json`; no game classes were executed.

The Java armor-glint software pass derives its equations, pipeline and sampler
contracts from cached Java 26.2 class/shader data and JOML 1.10.8. No game class
was loaded or executed. Optional `java_reference_jar` asset access reads only the
hash-pinned armor-glint PNG and metadata from a user-selected local archive;
it never places the archive on a class path, extracts assets or bundles them.
Static decoding of installed Bedrock actor/item glint PNGs found different RGBA
pixels; those textures are not substituted for Java artwork. Resource hashes and
unverified rendering/replay gaps are in `inspection/java-cosmetic-pass-26.2.json`.
This source-only pass does not establish native, GPU or deterministic parity.

JavaItemFoil independently decodes the cached Java 26.2 ItemStack.hasFoil and
Item.isFoil branch order. Items' Elytra registration and registerItem bootstrap
establish its default base Item constructor. Arctic's selector consumes explicit
adapter inputs and keeps unknown distinct from confirmed override absence; no
game class, virtual or component lookup is executed. The installed native
producer does not yet supply these Java inputs. Selector logic and archive
handling do not establish live glint activation or rendering parity.

The read-only enchantment observation cross-references pinned public declarations
for [ItemStackBase](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/world/item/ItemStackBase.cpp),
[CompoundTag](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/deps/nbt/CompoundTag.h),
[CompoundTagVariant](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/deps/nbt/CompoundTagVariant.h)
and [Tag](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/deps/nbt/Tag.h).
Installed getter/lookup/constructor instructions independently establish the
pinned offsets and type checks. Arctic's bounded read-only search invokes no
Minecraft/SDK function. Its separate raw-entry stage copies only bounded id/lvl
short fields into snapshots, as described below.
Enchantment-list presence alone does not establish native or Java glint behavior.

Optional enchantment-list header metadata also cross-references the pinned
[ListTag declaration](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/deps/nbt/ListTag.h).
Installed ListTag toString/equals and construction instructions independently
establish the vector stride/header and element-type byte. The header stage
interprets the already copied inline header, retaining size zero distinctly from
an absent tag. Cached Java
26.2 ItemStack/Item class data independently establishes the explicit glint
override and nonempty enchantment-component branches. A native list header is
not a validated Java component conversion or a rendered glint receipt.

The separately gated raw-entry reader cross-references pinned declarations for
[UniqueTagPtr](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/deps/nbt/UniqueTagPtr.h),
[ShortTag](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/deps/nbt/ShortTag.h)
and [ItemEnchants](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/world/item/enchanting/ItemEnchants.h).
Installed decoder/typed-lookup/short-copy instructions independently establish
entry pointer access, id/lvl key selection, the short type filter and value offset.
Only raw nullable shorts are retained; no SDK/game function is called and no
unrelated NBT, implementation or native address is included in snapshots.
The [Enchant::Type declaration](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/world/item/enchanting/Enchant.h)
also supplies the vanilla component-projection reference, with the installed
decoder independently bounding the effective low-byte ID at 41. Cached Java
26.2 Enchantments class data supplies corresponding default-namespace resource
keys; ItemEnchantments supplies constructor and codec level ranges. Arctic's
logical projection preserves raw inputs and declines unsupported duplicate/range
cases. It loads no game classes, copies no game implementation and changes no
inventory. Runtime conversion, complete overrides and glint-rendering equivalence
remain unverified.

The base glint selector additionally cross-references
[Item's flag/virtual declaration](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/world/item/Item.h)
and the cached Latite Item::isGlint slot declaration. Installed vtable entries,
a constructor's table store and the selected getter's actual stack-read/OR/mask
instructions independently establish the source binding. No virtual is invoked;
unknown override implementations are not treated as the base predicate.

Java wing animation/rig research independently decodes cached Java 26.2
ElytraAnimationState, Vec3.normalize, Mth.lerp, ElytraModel and WingsLayer class
data. native_wing_animation reconstructs the per-tick formulas; JavaWingPhysics
samples owned endpoints and JavaWingModel reconstructs the adult model's boxes
and part offsets. Native flags 1/32 are cross-referenced with the pinned
[ActorFlags declaration](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/world/actor/ActorFlags.h).
That enum reference is not runtime proof of Java crouch/fall-flying semantics.
No Minecraft/SDK implementation or game asset is bundled by these additions.

JavaCapeMesh/JavaModelBox independently reconstruct the dimensions, face-corner order and
UV mapping described by cached Java 26.2 PlayerCapeModel, CubeDefinition and
ModelPart.Cube/Polygon class bytes. CapeLayer and the cached entity shader and
pipeline classes establish the separate solid material's state. No Minecraft
classes were loaded or executed; no game binary, shader or texture is bundled
by this addition. The port remains unbuilt and untested.

The wing material derives from cached Java 26.2 WingsLayer,
EquipmentLayerRenderer, RenderTypes and RenderPipelines class data, plus the
cached vanilla elytra equipment definition. JavaModelBox applies ModelPart's
mirrored endpoint/UV/vertex-order rules to the separately documented Bedrock
wing boxes. That material port does not establish Java rig or native GPU parity.

JavaLayerProjection reconstructs cached Java 26.2 LayeringTransform and
ProjectionType behavior, with JOML 1.10.8 scale/translate operation order.
Class-file BootstrapMethods records establish each lambda target and direction;
RenderType.writeDynamicTransforms, GameRenderer and LevelRenderer establish
model-view modification before projection. No Minecraft/JOML classes are loaded
or executed. The installed renderer's existing camera observation supplies the
owned matrix pair; no new native function is called by this source addition.

JavaCapePose reconstructs the local setupAnim/rotateBy/translateAndRotate
sequence using cached JOML 1.10.8 Quaternionf, Matrix3f/Matrix4f, Math and Options
class instructions. Its default non-fast-math path retains the sine-derived
cosine, quaternion products, Euler extraction and rotation reconstruction.
It adds no JOML binary dependency and does not recover native parent pose history.

JavaNormalMatrix independently reconstructs the matrix cofactors and scalar
operation order from cached JOML 1.10.8 Matrix3f.normal/transform and
Vector3f.normalize class bytes. JOML's default non-FMA path is the reference;
its binary is not included by this addition. The helper operates on the final
owned native pose and cannot recover the original game's separate pose-stack
normal matrix or trusted-normal history. This source has not been built or run.

The local skinned-material copy binding checks the public
[FrameBuilder declaration](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src-client/mc/deps/minecraft_renderer/framebuilder/FrameBuilder.h)
as an attribution reference for void insert calls and reference-wrapper variants.
Its virtual-slot positions differ from the installed image and are not copied.
Installed profiler strings, pointer records, constructor stores, the actual
vtable entry, variant switch, local constant stores and helper call independently
establish the pinned source binding. No SDK implementation or game code/assets
are distributed. Only bounded native caller-stack float fields are temporarily
overridden; shared shader globals are not edited. This addition is unbuilt and
untested, with GPU execution and full material fidelity unverified.

The stripped-shader metadata fallback independently reads BGFX version 5 uniform
wrappers from the installed Actor material. The public
[BGFX HLSL shader writer](https://github.com/bkaradzic/bgfx/blob/master/tools/shaderc/shaderc_hlsl.cpp)
and the cached D3D12 backend reader describe the uniform name/type/array, byte
offset and register count records retained separately from stripped D3D shader
reflection. Arctic's bounded reader checks its pinned asset fingerprint, DXBC
declarations and exact shader hash, and exports vector records only. No BGFX or
Lazurite implementation, shader bytecode or game asset is copied or distributed.
Installed shader instructions were inspected as data using the Windows system
D3DDisassemble API; no game or shader was executed. This source addition has not
been built or tested.

The read-only dimension binding uses the cached Latite Actor.h shared-dimension
slot declaration as an attribution reference. Installed accessor disassembly
corroborates actor+0x1c8 and identifies the current name at dimension+0x48 and ID
at +0x78. The older Dimension.h name offset +0x20 is not used. Names/IDs are
copied under guarded reads and rechecks, with no accessor invocation or native
ownership transfer. The renderer's directional-light formulas and pairs were
ported from cached Java 26.2 shader/class data and its JOML 1.10.8 dependency;
those binaries are not included. This source has not been built or run.

The vendored hudhook DX11/DX12 texture backends have local source changes for
resizing an existing texture ID. They replace its owned resource/view in place
instead of allocating another permanent slot. The DX12 render context waits for
all buffered overlay draws before replacing a descriptor, and new resources
start in COPY_DEST with their existing uploaded-state tracking reset. Both
backends reject RGBA byte lengths that differ from the requested geometry.
These changes have not been built or run.

`src/native_equipment.rs` also uses Latite commit
`9f7463515dd298a496da918285936d78c7416aad` declarations for
[ItemStackBase](https://github.com/LatiteClient/Latite/blob/9f7463515dd298a496da918285936d78c7416aad/src/mc/common/world/ItemStackBase.h),
[Item](https://github.com/LatiteClient/Latite/blob/9f7463515dd298a496da918285936d78c7416aad/src/mc/common/world/Item.h)
and [HashedString](https://github.com/LatiteClient/Latite/blob/9f7463515dd298a496da918285936d78c7416aad/src/mc/deps/core/StringUtils.h).
The same revision's
[ActorEquipmentComponent](https://github.com/LatiteClient/Latite/blob/9f7463515dd298a496da918285936d78c7416aad/src/mc/common/entity/component/ActorEquipmentComponent.h)
describes separate hand and armor container pointers. The read-only observer
copies bounded groups only when their getter RVA matches an already inspected
installed-image container layout. It retains native container order; it does not
infer an inventory selection or invoke SDK item/inventory methods. Full armor/
hand layout and rendering remain unverified in the installed running client.
They describe the weak item reference and three canonical name fields used for
an optional read-only chest-item identity. The guarded reader checks their
agreement and copies them again before returning owned text. No item getter,
inventory action, engine constructor or SDK implementation is invoked or copied.
These declarations guide this static source port; the optional name layout has
not been corroborated in the installed game and does not authorize suppression.

`lan_probe.py` uses Microsoft's public
[GetExtendedUdpTable](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getextendedudptable)
and [MIB_UDPROW_OWNER_PID](https://learn.microsoft.com/en-us/windows/win32/api/udpmib/ns-udpmib-mib_udprow_owner_pid)
and `MIB_UDP6ROW_OWNER_PID` declarations in the installed SDK's `udpmib.h` to
observe the owned game's IPv4/IPv6 UDP endpoints. Structure alignment
comes from ctypes rather than copied offsets. Socket option constants are checked
against the installed Windows SDK headers. No IP Helper implementation is copied.

The loopback probe's read-only address-list allowance cross-checks Microsoft's
[SIO_ADDRESS_LIST_QUERY documentation](https://learn.microsoft.com/en-us/windows/win32/winsock/sio-address-list-query)
and System Informer's public
[AFD declarations](https://github.com/winsiderss/systeminformer/blob/master/phnt/include/ntafd.h).
The installed Winsock query independently verifies the observed control code and
family argument. No driver implementation or leaked Windows source is used.

`src/native_gpu_constants.rs` uses the public Windows SDK resource Map/Unmap,
GetGPUVirtualAddress, GetDesc and GetHeapProperties declarations. The custom
heap distinction was checked against Microsoft's
[D3D12_HEAP_TYPE documentation](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/ne-d3d12-d3d12_heap_type)
and upstream BGFX's
[D3D12 heap initialization](https://github.com/bkaradzic/bgfx/blob/master/src/renderer_d3d12.cpp)
(consulted 2026-10-24). Upstream references guide inspection; they do not prove
the installed game's heap profile, shader layout or GPU results. No BGFX
implementation was copied into the diagnostic.

`native_shader_layout.rs` uses Windows SDK
[D3DReflect](https://learn.microsoft.com/en-us/windows/win32/api/d3dcompiler/nf-d3dcompiler-d3dreflect)
and the versioned root-signature
[deserializer](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/nf-d3d12-id3d12versionedrootsignaturedeserializer-getrootsignaturedescatversion).
Shader bytecode is copied only from successful native PSO creation arguments and
is not retained or distributed. Reflection names/offsets are declarations, not
proof that every value was executed by a GPU. `native_scene_target.rs` uses public
D3D12 resource private data for lifetime IDs, avoiding retained swap-chain buffers.
DXIL uses the installed Windows SDK's dynamically resolved
[IDxcUtils::CreateReflection](https://learn.microsoft.com/en-us/windows/win32/api/dxcapi/nf-dxcapi-idxcutils-createreflection).
Root signatures with
[visibility ALL](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/ne-d3d12-d3d12_shader_visibility)
include vertex and pixel access; register/space values are still read from the
actual deserialized object. The SDK compiler is not bundled or downloaded.
Local inspection artifacts extracted from installed material files are kept
only under the ignored probe output directory, not the distributable bundle.

`analyze_bindings.py` adapts five signature byte patterns from
[LatiteClient/Latite](https://github.com/LatiteClient/Latite),
`src/mc/Addresses.h`, commit `9f7463515dd298a496da918285936d78c7416aad`.
Latite is licensed under GPL-3.0; Arctic uses GPL-3.0-or-later. The scanner does
not distribute any Minecraft executable bytes or symbols. Inspection output is
local and ignored under `target/`.

`src/world_hooks.rs` uses that revision's world/actor/tick callback signatures
and platform/renderer field offsets (`Platform_GameCore.cpp`, `MinecraftGame.h`,
`GameRenderer.h`, `LevelRenderer.h`, `LevelRendererPlayer.cpp`). The runtime
profile is pinned to the independently inspected executable's SHA-256. Guarded
memory reads and observer plumbing are Arctic code; observations are not a
complete cosmetics adapter or proof of live Minecraft behavior.
The actor callback's void return type, reference arguments and final
`ignoreLighting` boolean are cross-checked against the SDK's
[ActorRenderDispatcher](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/actor/ActorRenderDispatcher.h)
declaration and the exact installed dispatcher's argument use.

`src/players.rs` uses that SDK's `Level.cpp`, `Player.h`, `PlayerListEntry.h`,
`SerializedSkinRef.h`, and actor component layouts. `tests/player_layout.cpp`
independently checks the MSVC release container ABI. `src/ecs.rs` implements
guarded storage traversal checked against the user's installed executable,
including the 0x250-byte identity component and its UUID at +0xB8. No game bytes
are included. Component names use EnTT's FNV-1a hashes. Additional identity layout
and actor flag references: [LeviLamina's public SDK](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/entity/components/UserEntityIdentifierComponent.h).

Player samples now join UUIDs directly through EnTT, including the SDK's
`StateVectorComponent`, `ActorRotationComponent`, `AABBShapeComponent`,
`OnGroundFlagComponent`, and `IsDeadFlagComponent`.
`src/animation_pose.rs` reads owned observations using that public SDK's
`AnimationComponent`, `RenderParams`, and `BoneOrientation` field declarations.
The installed player's animation getter passes its shared pointer at +0x270
(RVA 0x210c69) to `Actor::_getAnimationComponent` (RVA 0x1A28250). The native
`updateBoneTransform` declaration names its matrix output bone-to-entity space;
the installed routine at `0x1BC3A30` corroborates the cached matrix stores. Six
recorded standard player matrices inform a checked standing attachment mapping.
This is not treated as a finished body/cape replacement.

Deferred draw-stack analysis compares the installed batch flush instructions with
[BGFX's D3D12 backend](https://github.com/bkaradzic/bgfx/blob/master/src/renderer_d3d12.cpp)
and its [frame/context declarations](https://github.com/bkaradzic/bgfx/blob/master/src/bgfx_p.h).
Only public declarations and independently observed ABI behavior inform Arctic;
BGFX implementation files are not bundled. Mojang's public GDC 2026 talk,
[Modernizing the Rendering of Minecraft](https://media.gdcvault.com/gdc2026/Slides/Fairfield_AJ_ModernizingTheRenderingOfMinecraft.pdf),
describes the separate RenderDragon frame registry and BGFX abstraction. This
explains the need for a proven frame handoff; it does not supply build offsets
or authorize associating unrelated draws with a world pass.
The submission bridge cross-checks the SDK's
[Frame](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/external/bgfx/Frame.h),
[Context](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/external/bgfx/Context.h),
[EncoderImpl](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/external/bgfx/EncoderImpl.h)
and [View](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/external/bgfx/View.h)
declarations against native stores, copies and accesses in the exact installed
image. It forwards all native calls and does not execute copied engine code.

`src/native_capes.rs` independently implements the same following filter and
local movement/bob inputs inspected in the user's cached Java 26.2 classes
(`ClientAvatarState`, `AbstractClientPlayer`, and `LocalPlayer`). It runs only
after completed primary-client level ticks. Teleports reset interpolation to
avoid unbounded displacement. Minecraft class files/source are not distributed.

`src/native_input.rs` uses `MoveInputComponent`/`MoveInputState` declarations
from the same pinned Latite commit, corroborated by the cached installed-image
accessors at RVAs `0x28348f0`, `0x28349d0` and `0x2834b90`. The public
[LeviLamina component declaration](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/entity/components/MoveInputComponent.h)
also documents the processed input and jump/sneak flag roles. That declaration
alone does not establish the installed layout. The new read-only observer uses
Arctic's checked EnTT traversal and retains copied values, never a borrowed
component pointer. No game code is copied into the distributable source.

`src/native_packet.rs` cross-checks the four-argument dispatcher callback ABI
against the same pinned Latite commit's `PacketHooks.h`/`PacketHooks.cpp` and
`Packet.h`. Arctic observes an independently resolved installed-image latency
dispatcher after its original return. It does not copy Latite's packet creation,
vtable swapping, event dispatch or cancellation implementation. Static factory
cases, constructor stores and virtual thunks establish the exact image offsets
recorded in `inspection/native-packet.json`; private game bytes remain only in
the ignored local inspection directory. No native packet method is called to
discover an ID, create a packet, or take a reference-counted ownership copy.

The primary-player weak-reference reader cross-checks the pinned client's
`getLocalPlayer` / weak-lock / component accessors at RVAs `0x5DAED30`,
`0x25C0380` and `0x476E930` against the SDK's
[WeakEntityRef](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/deps/ecs/WeakEntityRef.h),
[EntityContext](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/deps/ecs/gamerefs_entity/EntityContext.h)
and [ActorOwnerComponent](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/entity/components/ActorOwnerComponent.h)
declarations. It copies and validates registry/entity state without invoking
engine constructors, incrementing engine reference counts or copying game code.

Registry lifetime also uses the SDK's
[EntityRegistry](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/deps/ecs/gamerefs_entity/EntityRegistry.h)
`mId` declaration, independently checked against the installed constructor's
atomic fetch-add/store (`0x25bfcd5` / `0x25bfcdd`, owner+0x160) and native
EntityContext comparisons (`0x19fcdbd` / `0x19fcdca`). This ID separates actual
registry replacement from stale cached player data; no engine constructor or
reference-count operation is called.

The optional read-only flipbook observer cross-checks the SDK's
[FlipbookTexture::createDescription](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/ptexture/FlipbookTexture.h)
declaration against the installed routine at `0x661b740` and the exact
[TickingTextureStage](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/game/TickingTextureStage.h)
caller at `0x46e8e4e`. Native loads/stores establish the phase/count/period input
offsets and returned frame/blend fields. Public declarations supply no assumed
implementation. The observer neither copies native code nor changes textures.
The SDK's [ScreenContext clock reference](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/gui/screens/ScreenContext.h)
is independently located at +0xb0 by the installed constructor store at
`0x4816018`; the exact makeScreenContext passes renderer+0x1f0 as that argument.
Clock ownership diagnostics do not establish frame ownership.

`src/native_shader_inputs.rs` cross-checks the SDK's
[GlobalConstantBuffers](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/constantbuffers/GlobalConstantBuffers.h),
[PerFrameConstants](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/constantbuffers/PerFrameConstants.h),
[InterFrameConstants](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/constantbuffers/InterFrameConstants.h)
and ShaderConstantBase declarations against installed name/type checks at
`0x595ce10`, matrix checks at `0x595a0d2` and indirect data accesses at
`0x595ae59` / `0x47a9336`. It copies only recognized named constants, without
invoking methods or changing shader state. No renderer implementation is copied.

The controlled camera-delta adapter uses the SDK's
[LevelRendererCamera](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/game/LevelRendererCamera.h)
field declarations, corroborated by the installed positive-delta branch at
`0x46856c2` through `0x46856e4`. Its three-argument pre-render signature is checked
against the declaration and native argument use. The same declared layout puts
camera target position at +0x660, independently verified by the existing matrix
consumer. No SDK renderer implementation is included.
The total shader-time adapter checks the exact executable's fmod import name,
thunk and direct caller, preserving the Windows x64 double ABI and native hour
modulus. It does not patch the process-wide CRT export.

`src/native_skins.rs` uses the SDK's `SerializedSkinImpl`, `SkinImage`,
`mce::Image`, `mce::Blob`, `ThreadOwner`, and `ArmSizeType` declarations to copy
classic RGBA8 skin/cape assets with checked sizes. Persona/custom geometry is
kept in the native renderer. These readers do not grant access to Marketplace
assets or suppress the original renderer.
The cape absence and armor observations cross-check `SerializedSkinImpl`,
`ActorEquipmentComponent`, `SimpleContainer`, `ItemStackBase`, and `ItemStack`
declarations against the installed getter instructions. Empty-item comparison
uses the actual getter fallback object without constructing or calling it.
The distinction between model-pixel units and global resource scale is checked
against `RenderParams::init`, the installed Molang query callback, and Mojang's
[public player client entity](https://github.com/Mojang/bedrock-samples/blob/main/resource_pack/entity/player.entity.json).

The vendored hudhook/MinHook licenses and Arctic callback changes are retained
under `vendor/hudhook/`. FFmpeg is an external user-selected executable and is
not bundled or downloaded by the native adapter.

`examples/inspect_debug_chain.rs` reads only the normalized x64 process
parameters' `DesktopInfo` field when User32 has not attached the initial thread.
The field layout is documented in [phnt's ntrtl.h](https://github.com/winsiderss/phnt/blob/master/ntrtl.h).
Default inspection terminates its own debuggee at process creation. The opt-in
isolated running mode resumes only after the test guards initialize. DesktopInfo
reading is shared with the bounded live-stack diagnostic; neither reader
inspects command lines, environments or account data.

`probe_isolation.rs` uses public NT file API layouts documented by Microsoft's
driver reference and [phnt's ntioapi.h](https://github.com/winsiderss/phnt/blob/master/ntioapi.h).
The GDK `QueryApiImpl` ABI and user-provider CLSID come from
[xodus-gaming/xgameruntime-docs](https://github.com/xodus-gaming/xgameruntime-docs/blob/main/QueryApiImpl.md)
and its `COM/XUserImpl/README.md` (GPL-3.0). The test denies this provider; it
does not implement platform authentication or licensing. AFD IOCTL encoding is
corroborated by [Wine's public afd.h](https://github.com/wine-mirror/wine/blob/master/include/wine/afd.h).
No Wine or GDK binary implementation is distributed.
The optional loopback test guard uses public Winsock signatures and address
layouts, plus AFD operation numbers cross-checked with
[ReactOS's shared AFD header](https://github.com/reactos/reactos/blob/master/sdk/include/reactos/drivers/afd/shared.h).
Its address validation and call-scope implementation are Arctic code; no AFD
driver implementation is copied or distributed.
The installed OS's provider-helper connect shape and scalar transport options
are cross-checked against [phnt's AFD declarations](https://github.com/winsiderss/phnt/blob/master/ntafd.h)
(`AFD_CONNECT_JOIN_INFO_TL`, `AFD_TL_IO_CONTROL_INFO`) and the local Windows SDK's
Winsock constants. The headless Java lookup test independently exercises these
requests. Only the exact checked loopback socket/address is accepted; no provider
or driver implementation is copied.
Read-only NSI query classification is corroborated by Project Zero's
[NsiGetParameter report](https://issues.chromium.org/issues/42450260) and
[Dr. Memory's native IOCTL decoder](https://github.com/DynamoRIO/drmemory/blob/master/drsyscall/drsyscall_windows.c),
then exercised through Windows `GetIfEntry` and the initialized GDK connectivity
query without copying decoder code.
Connectivity hint fields and callback signatures follow Microsoft's
[XNetworkingConnectivityHint](https://learn.microsoft.com/en-us/gaming/gdk/docs/reference/networking/xnetworking/structs/xnetworkingconnectivityhint)
declaration. The pinned game's own static wrappers corroborate the 16-byte layout,
initializer version arguments and observer ABIs. The optional fixture invokes the
installed runtime's initializer; no runtime implementation is bundled.

The optional empty-user test provider uses the same published COM method
layouts. It implements only synthetic signed-out failures and does not call
the real user provider. Async block, provider-operation and task-queue ABI
declarations are checked against Microsoft's MIT-licensed
[libHttpClient public headers](https://github.com/microsoft/libHttpClient/tree/main/Include)
(`XAsync.h`, `XAsyncProvider.h`, `XTaskQueue.h`). The real system GDK threading
provider delivers completion callbacks; no GDK implementation is bundled.
Public method signatures are also checked against
[WineGDK's XUser interface declarations](https://github.com/Weather-OS/WineGDK/blob/master/include/xuser.idl)
(LGPL-2.1-or-later). No authentication implementation is copied.

Passive loading diagnostics cross-check the installed code against LeviLamina's
[ProgressHandler](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/gui/ProgressHandler.h)
and [GameServerConnectProgressHandler](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/gui/GameServerConnectProgressHandler.h)
declarations. Disconnect stages and numeric reasons are checked against its
[DisconnectFailReason](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/network/connection/DisconnectFailReason.h)
and DisconnectionStage declarations. Only declarations inform these observers;
no game or SDK implementation is copied.
The bounded startup check also uses the SDK's
[IClientInstance](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/game/IClientInstance.h)
user-getter declaration and
[IdentityType](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/identity/IdentityType.h)
enum to classify pending operations. It copies operation flags and code RVAs,
never account names, identifiers, credentials or tokens.

Native transform observers cross-check the installed call sites with
[DataDrivenModel::render](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/model/models/DataDrivenModel.h),
[BaseActorRenderContext](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/BaseActorRenderContext.h),
[mce::Camera](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/renderer/Camera.h)
and [MatrixStack](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/renderer/MatrixStack.h).
These declarations inform ABI and layout checks; no SDK implementation is copied.

Native child-attachment binding also cross-checks LeviLamina's
[AnimationComponent and ChildAnimationComponentInfo](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/actor/animation/AnimationComponent.h),
[AttachableSlotIndex](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/actor/animation/AttachableSlotIndex.h)
and the DataDrivenModel attachment-method declarations above. Child slot stride,
shared-pointer location, selected controller list and draw call chain are derived
independently from the cached exact-image executable. The implementation copies
bounded metadata and matrices; no SDK/game attachment implementation is bundled.
The copied bone-binding flags and indexed parent semantics also cross-check
[BoneOrientation](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/actor/animation/BoneOrientation.h)
and [BoneBindingMethod](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/actor/animation/BoneBindingMethod.h).
The method-2 root lookup, 0xe8 stride, sixteen matrix stores and override flag
are independently traced in the cached installed image. The expression-binding
branch has different behavior; its old indices are not used as a parent receipt.
The constant-hash reader also cross-checks the declarations of
[ConstantExpression](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/util/molang/ConstantExpression.h),
[IComplexExpression](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/util/molang/IComplexExpression.h)
and [MolangScriptArgType](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/util/MolangScriptArgType.h).
Its vtable, stored-value offsets and root-query vector getter are independently
derived from cached machine code. It reads an already stored constant hash;
no Molang implementation or SDK implementation is copied or executed.

The native skin observer also checks the declarations of
[Model::dataDrivenDraw](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/model/models/Model.h),
[RenderController](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/controller/RenderController.h)
and [RenderParams](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/actor/RenderParams.h).
The logical `default` geometry alias is corroborated by Mojang's
[player render controllers](https://github.com/Mojang/bedrock-samples/blob/main/resource_pack/render_controllers/player.render_controllers.json).
The passive render-item observer uses the public
[bgfx encoder submission implementation](https://github.com/bkaradzic/bgfx/blob/master/src/bgfx.cpp)
as context; its installed ABI, fixed-array layout and counter offsets are checked
independently against the hashed executable. Current upstream bgfx uses different
frame storage. No engine or bgfx implementation is copied or bundled.

Skinned-command research also checks the
[SkinnedMesh](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/model/models/SkinnedMesh.h),
[SortedMeshDrawList](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/model/models/SortedMeshDrawList.h),
[MeshRenderData](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/model/models/MeshRenderData.h),
[MeshContext](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/renderer/MeshContext.h)
and [RenderSkinnedMeshDescription](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/deps/minecraft_renderer/framebuilder/RenderSkinnedMeshDescription.h)
declarations. The last declaration supplies a type name only. The installed
constructor ABI and color/matrix offsets are derived from its actual machine
code and call site, not inferred from that empty public declaration.

Native timer research cross-checks the public
[Timer](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/util/Timer.h) and
[Minecraft](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/Minecraft.h)
declarations. The installed `advanceTime` ABI, stepping branches, constants and
offsets are independently checked in the hashed executable; no SDK timer
implementation is copied or bundled.

Renderer-clock research also checks
[GameRenderer](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/game/GameRenderer.h)
and [mce::Clock](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/deps/core/timing/Clock.h).
The public clock declaration leaves its fields untyped. Clock meanings, native
updates, the context-builder ABI and the allowed call site are derived from the
installed executable rather than inferred from those unnamed fields.

Screen-context interpolation research cross-checks the public
[ScreenContext](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/gui/screens/ScreenContext.h)
and [UIScreenContext](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/gui/screens/UIScreenContext.h)
declarations. The float argument's actual constructor store at offset +8 is
checked independently in the pinned executable. Water/texture research consults
[TickingTextureStage](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/game/TickingTextureStage.h),
[FlipbookTexture](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/ptexture/FlipbookTexture.h)
and the renderer constant-buffer declarations; no SDK implementation is copied.

The headless test debugger's narrowly scoped encoder notification handling follows
Microsoft's documented
[native thread-name exception protocol](https://learn.microsoft.com/en-us/visualstudio/debugger/how-to-set-a-thread-name-in-native-code).
The implementation validates the observed x64 metadata layout and only handles
first-chance notifications from the explicitly configured owned encoder.

Camera research checks the public
[MinecraftCamera::CameraComponent](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/deps/minecraft_camera/components/CameraComponent.h)
and [VanillaCamera::UpdatePlayerFromCameraComponent](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/vanilla_camera/components/UpdatePlayerFromCameraComponent.h)
declarations, plus Latite's previously attributed camera-updater signature and
three-argument hook declaration. The installed storage lookup independently
uses type hash `0x4f6047c7` and stride `0x120`. The weak-entity selection and
updater call sites are checked in the hashed executable. No SDK camera
implementation or third-party client is bundled or loaded.

Native fog field research also checks the public
[LevelRendererCamera](https://github.com/LiteLDev/LeviLamina/blob/4ca13569fc0d1cdeb873cf6e7c48ddcc7a74e59c/src-client/mc/client/renderer/game/LevelRendererCamera.h)
scalar declarations. The installed method's ABI, vtable reference, scalar stores
and fixed per-render blends were independently inspected in the pinned image.
The empty public pre-render parameter declaration supplies no usable field layout.
Arctic restores only its bounded scalar replay history and invokes the original
fog calculation; no game or SDK fog implementation is copied or bundled.

The camera consumer research also checks the public
[mce::Camera](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/renderer/Camera.h)
and [EntitySystems](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/entity/systems/EntitySystems.h)
declarations. The render-tag lookup, matrix helper and field/caller meanings
are independently derived from the pinned executable, including the native
first-packed-tag selection and generation checks. No game matrix implementation
is copied into Arctic; the original engine computes its matrices.

Wing geometry/UV research uses Mojang's Bedrock Samples release v1.26.50.4:
[mobs.json, geometry.elytra](https://github.com/Mojang/bedrock-samples/blob/v1.26.50.4/resource_pack/models/mobs.json)
and [elytra.animation.json](https://github.com/Mojang/bedrock-samples/blob/v1.26.50.4/resource_pack/animations/elytra.animation.json).
Arctic's wing emitter uses the documented dimensions and cape-atlas UV layout,
with explicit right-wing mirroring. Native observation retains completed engine matrices,
including nonuniform scale; no Molang controller implementation, textures or
game binary is copied or bundled. These references do not establish that the
installed game's selected geometry or bone matrices match the observation path.

Installed archive research cross-checks the project's own
[brarchive format description](https://github.com/bedrock-crustaceans/brarchive/blob/main/FORMAT.md)
against the installed `textures/items.brarchive` header. `PackAssets` is an
independent bounded reader for that layout, including arbitrary binary payloads;
no third-party parser implementation, game assets or extracted files are bundled.
The referenced format is independently documented rather than an official Mojang
specification. Matching the header does not establish runtime pack selection or
successful decoding of the installed item atlas.

Optional item icon metadata research also checks LeviLamina's public
[Item declaration](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/item/Item.h).
The atlas and icon-name strings are copied from the prefix whose canonical-name
anchors match the existing observer; this is source evidence, not validation of
the installed client's layout or per-stack resolved icon variant. No SDK getter
or renderer is invoked. Static catalog handling cross-checks Mojang's
[item texture reference](https://github.com/Mojang/bedrock-samples/blob/v1.26.50.4/resource_pack/textures/item_texture.json)
and Microsoft's [custom item guide](https://learn.microsoft.com/en-us/minecraft/creator/documents/addcustomitems?view=minecraft-bedrock-stable),
which includes a catalog without texture_name. Arctic copies no sample images
or catalog entries and does not infer the selected variant of an atlas array.

Resource-stack research uses LeviLamina's public declarations for
[ResourcePackManager](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/resources/ResourcePackManager.h),
[ResourcePackStack](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/resources/ResourcePackStack.h),
[PackInstance](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/resources/PackInstance.h),
[ResourcePack](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/resources/ResourcePack.h),
[Pack](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/resources/Pack.h) and
[DirectoryPackAccessStrategy](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/resources/DirectoryPackAccessStrategy.h).
Latite's previously attributed ClientInstance declaration identifies the resource
delegate slot. Arctic independently derives the installed client tables, wrapper
indirection, full-stack/child/overlay/base lookup order and resolved-directory
field from offline inspection of the pinned cached image. The public declarations
alone do not authorize client offsets: the client ResourcePack wrapper differs
from the direct server layout. Only semantic field facts and addresses are
retained; no SDK implementation, game binary, resource file or asset extraction
is bundled. No game getter or loader is called by this read-only observer, and
its unbuilt source is not evidence of runtime stack or rendering parity.

Static icon-frame research also checks the public
[ItemIconInfo](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/item/ItemIconInfo.h),
[ResolvedItemIconInfo](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/item/ResolvedItemIconInfo.h)
and [ItemIconInfoType](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/item/ItemIconInfoType.h)
declarations. Arctic independently inspects the installed base selector and
requires its component/armor/alternate-atlas branches to be excluded before
copying a key/frame pair. The stored frame is not inferred from item names or
stack damage. No getter implementation, sample texture or catalog is copied,
invoked or bundled; binary branch evidence is not runtime rendering validation.

Subchunk work observation cross-checks LeviLamina's public
[ClientLevel](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/multiplayer/ClientLevel.h),
[SubChunkManager](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/multiplayer/SubChunkManager.h),
[SubChunkInsertManager](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/network/SubChunkInsertManager.h),
[TaskGroup](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/deps/core/threading/TaskGroup.h) and
[TaskGroupState](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/deps/core/threading/TaskGroupState.h)
declarations. The installed layouts, constructor stores, vtables, queue stride
and task-count loads are independently derived from the pinned cached image;
SDK field order alone does not authorize offsets. The observer copies bounded
heap metadata without calling a getter, scheduler or task function. No native
implementation or binary is copied or bundled. Equal endpoint observations do
not establish complete application, terrain, mesh or GPU readiness.

Task activity observation also checks the public
[IBackgroundTaskOwner](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/deps/core/threading/IBackgroundTaskOwner.h)
virtual declaration order and the TaskGroup constructor/destructor declarations
linked above. Arctic independently derives the pinned register/complete slots,
their native counter updates, constructor this-return and the shared destructor
body from cached executable bytes. It forwards native shared-pointer/string
argument storage without adopting its ownership and retains no task address.
No scheduler or task implementation is copied, called separately or bundled.

Network chunk-work research checks LeviLamina's public
[GameSession](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/GameSession.h),
[ClientNetworkHandler](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/network/ClientNetworkHandler.h)
and [LegacyClientNetworkHandler](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/network/LegacyClientNetworkHandler.h)
declarations. Cached executable decoding independently establishes the client
session's owned callback, its final legacy vtable, client/level references,
pending-map headers and owned I/O TaskGroup factory/name. Arctic reads bounded
metadata without invoking a network, resource, account or packet handler. No
game implementation or binary is copied or bundled, and these declarations and
reads do not establish complete chunk application or terrain readiness.

Legacy insertion research also checks LeviLamina's public
[NetworkChunkInserter](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/network/NetworkChunkInserter.h)
and [LevelChunkPacketHandler](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/network/LevelChunkPacketHandler.h)
declarations. Those declarations do not supply the client field layout. Arctic
independently derives the 48-byte owned queue, owner back-reference, issued and
consumed sequences, ready-vector stride and consume/discard behavior from the
pinned cached executable. Only bounded queue metadata is copied by the observer;
no chunk payload, native implementation, SDK implementation or game binary is
bundled. Consumed accounting is not a successful-application or terrain receipt.

Renderer task observation cross-checks the public
[LevelRenderer](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/game/LevelRenderer.h)
and [RenderChunkCoordinator](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/chunks/RenderChunkCoordinator.h)
declarations and the TaskGroup declarations linked above. Arctic independently
derives both renderer tables, owned group offsets/names, client/level references
and the visibility rebuild's use of the background group from cached executable
bytes. The observer calls no engine getter, scheduler, renderer or task function
and retains no task pointer. No SDK implementation or game binary is bundled;
idle observed groups do not establish terrain or GPU completion.

Current-dimension chunk-change research additionally checks the public
[LevelRendererCamera](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/game/LevelRendererCamera.h)
and [Dimension](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/world/level/dimension/Dimension.h)
declarations. Cached constructor, factory and lookup code independently supplies
the current camera's dimension link, trivial dimension-ID getter field, owned
coordinator map layout and pending map headers. Arctic copies bounded metadata
without executing the getter, changing reference counts, retaining tasks/chunks
or calling a coordinator method. No native implementation or SDK implementation
is copied or bundled; these observations do not establish successful terrain
convergence or GPU completion.

Shared-chunk build observation checks the public
[RenderChunkShared](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/chunks/RenderChunkShared.h)
declaration and its BuildState/VisibilityBuildState enums. Arctic independently
derives the pinned coordinator weak-map layout, in-place shared owner table,
chunk-position stores, constructor-assigned ID and state transitions from the
cached executable. The observer copies bounded list/state/identity metadata and
reference liveness without modifying reference counts or invoking a constructor,
getter, build, sort, visibility or renderer function. No SDK implementation,
chunk/geometry payload or game binary is copied or bundled. Observed idle states
are not evidence of the visible set, complete terrain or GPU readiness.

World-argument observation checks the public
[FrameRenderObject](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/objects/FrameRenderObject.h),
[ViewRenderObject](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/objects/ViewRenderObject.h),
[ViewRenderData](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/objects/ViewRenderData.h),
[ClientRenderData](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/objects/ClientRenderData.h),
[ChunkRenderObjectCollection](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/objects/ChunkRenderObjectCollection.h),
[ChunkRenderData](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/objects/ChunkRenderData.h)
and [ChunkLayerRenderObject](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/objects/ChunkLayerRenderObject.h)
declarations. Arctic independently derives the borrowed world argument, vector
allocator prefix, view stride, camera/client fields, collection offsets and queue
strides from cached producer/move/destructor code. It calls no engine function,
modifies no frame/resource/reference count and copies no geometry/resource payload
or SDK implementation. Equal metadata endpoints do not prove input consumption,
complete projection, visible geometry or GPU execution.

Selected-view entry research additionally checks the public
[TerrainLayerLOD](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/game/TerrainLayerLOD.h)
declaration and the ChunkRenderData/ChunkLayerRenderObject declarations linked
above. Cached producer/destructor code independently supplies the position/time
stores, mesh/index variant selectors, shared-reference storage, layer index,
material-pointer field, index ranges, camera mask, fallback and LOD byte stores.
Arctic copies bounded borrowed records and observes positive shared-control
counts without changing ownership, invoking native code or reading geometry/GPU
buffers. Monostate union storage is ignored; resource aliases are preserved.
No SDK implementation or game binary is bundled. These metadata observations
do not establish resource types, material ownership, buffer bounds, actual draw
selection or upload completion.

Direct-buffer availability additionally checks the public
[RenderChunkDirectVertexData](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/game/RenderChunkDirectVertexData.h)
and [RenderChunkDirectIndexData](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/game/RenderChunkDirectIndexData.h)
declarations. The installed queue producer independently establishes its vertex,
attribute, split-stream and index optional-field tests. Arctic copies and guards
those fields without interpreting ServerResourcePointer storage, calling the
producer or modifying native state. Optional availability does not establish
buffer contents/bounds, server-resource resolution or GPU upload completion.

Ordinary-resource availability checks the public
[Mesh](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/renderer/Mesh.h),
[MeshData](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/renderer/MeshData.h),
[IndexBufferContainer](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/minecraft_renderer/resources/IndexBufferContainer.h),
[ClientResourcePointer](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/deps/core/checked_resource_service/ClientResourcePointer.h)
and [ResourcePointer](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/external/render_dragon/resources/ResourcePointer.h)
declarations. Arctic independently derives the producer's mesh validator,
CPU-vector fallback fields, opaque attribute condition, tracking-flag bit,
polymorphic header table and shared-control offsets from cached native
instructions. No SDK method body or inferred SDK implementation is copied or
invoked. Only bounded metadata/reference guards are retained privately; no CPU
vertex/index array, backend buffer or game binary is bundled. These predicates
do not establish actual rendering, completed upload or resource payload validity.

Layer material-reference research checks the public
[TerrainMaterialVariationManager](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/client/renderer/game/TerrainMaterialVariationManager.h),
[MaterialPtr](https://github.com/LiteLDev/LeviLamina/blob/main/src/mc/deps/minecraft_renderer/renderer/MaterialPtr.h)
and [RenderMaterialInfo](https://github.com/LiteLDev/LeviLamina/blob/main/src-client/mc/deps/core/renderer/RenderMaterialInfo.h)
declarations. Cached layer production independently establishes the terrain
map's value address and its storage in a layer; the native material constructor
establishes the returned shared pair and strong/weak control handling. Arctic
copies and guards those borrowed pairs without traversing a guessed material-info
layout, invoking the group lookup, taking a native lease or modifying references.
No SDK implementation or game binary is bundled. Shared-reference liveness is
not evidence of resolved shader identity, native resource ownership or rendering.

Avatar geometry upload research reads the public
[Mesh](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src-client/mc/deps/minecraft_renderer/renderer/Mesh.h),
[MeshData](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src-client/mc/deps/minecraft_renderer/renderer/MeshData.h),
[BufferResourceService](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/deps/minecraft_renderer/resources/BufferResourceService.h)
and [BufferResourceServiceContext](https://github.com/LiteLDev/LeviLamina/blob/310bf95e173df01bc08a490dbdcb2e8dd0ae1de2/src/mc/deps/minecraft_renderer/resources/BufferResourceServiceContext.h)
declarations. Cached instructions independently establish constructor argument
handling, MeshData copy/move ownership, weak-service locking and preparation
call sites. No SDK implementation is copied or invoked, and the opaque service
layout remains unresolved. NativeAvatarGeometry stages Arctic's existing shared
models in owned Java arrays; it does not construct an engine Mesh or transfer
those allocations to a native destructor. No game binary or cached instruction
payload is added to the distributable by this source port.

The explicit-input avatar material port studies the user's cached Java 26.2
client shader resources (entity vertex/fragment, lighting, fog, lightmap sampling,
dynamic transforms and projection) and statically decoded pipeline/blend class
bytes. The archive and individual asset hashes are recorded in
inspection/native-avatar-material-source.json. Arctic's HLSL and D3D12 bindings
independently express the observed shader calculations and pipeline states; no
Java shader file, game class, texture archive or compiler output is bundled by
this source change. The live mapping from Bedrock lighting/fog/overlay data
remains unresolved and is not substituted with invented Java constants.
