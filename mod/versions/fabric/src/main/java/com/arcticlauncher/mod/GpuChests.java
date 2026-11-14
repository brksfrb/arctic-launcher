// i18n-ignore-file: shader names, GPU labels and log lines; nothing here is shown to the player.
package com.arcticlauncher.mod;

//#if MC == 26.2
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.BlockEntityCulling;
import com.arcticlauncher.client.feature.ChestInstancing;
import com.mojang.blaze3d.GpuFormat;
import com.mojang.blaze3d.PrimitiveTopology;
import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.pipeline.BindGroupLayout;
import com.mojang.blaze3d.pipeline.ColorTargetState;
import com.mojang.blaze3d.pipeline.DepthStencilState;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.platform.PolygonMode;
import com.mojang.blaze3d.shaders.ShaderSource;
import com.mojang.blaze3d.shaders.ShaderType;
import com.mojang.blaze3d.shaders.UniformType;
import com.mojang.blaze3d.systems.GpuDevice;
import com.mojang.blaze3d.systems.RenderPass;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.textures.GpuTextureView;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexFormat;
import java.lang.reflect.Constructor;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.OptionalDouble;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.model.object.chest.ChestModel;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.ShaderDefines;
import net.minecraft.client.renderer.Sheets;
import net.minecraft.client.renderer.blockentity.BrightnessCombiner;
import net.minecraft.client.renderer.blockentity.ChestRenderer;
import net.minecraft.client.renderer.blockentity.state.ChestRenderState;
import net.minecraft.client.renderer.rendertype.PreparedRenderType;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.model.sprite.SpriteId;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.resources.Identifier;
import net.minecraft.util.LightCoordsUtil;
import net.minecraft.world.level.block.ChestBlock;
import net.minecraft.world.level.block.CopperChestBlock;
import net.minecraft.world.level.block.DoubleBlockCombiner;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.ChestBlockEntity;
import net.minecraft.world.level.block.entity.EnderChestBlockEntity;
import net.minecraft.world.level.block.entity.LidBlockEntity;
import net.minecraft.world.level.block.entity.TrappedChestBlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.ChestType;
import net.minecraft.world.phys.Vec3;
import org.joml.Vector3f;
import org.lwjgl.system.MemoryUtil;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Chests drawn on the GPU (26.2): the game's chests, trapped chests, ender chests and copper chests,
 * single and double, as three instanced draws a frame instead of a renderer call, a render state and
 * 72 vertices each.
 *
 * <p>What is drawn is kept by {@link ChestInstancing}: a chest with its lid shut sits in its section's
 * block of instances, written once; only a chest whose lid moves is written each frame. A chest being
 * broken is left to the game (which draws the cracks on it). The chest models' shapes are taken from
 * the game's own baked models and uploaded once; the vertex shader is made from the game's own entity
 * shader (as it is loaded, resource packs included) with only its inputs worked out per instance, and
 * the draw uses the game's own render type for chests (same fragment shader, textures, light, fog and
 * depth), so the pixels are the game's.
 *
 * <p>The game (or Sodium) still decides which sections' block entities are drawn; this takes each
 * chest at the block entity dispatcher, before its render state is made. Stays off with shader mods,
 * mods that change chest models or textures, portal mods, or when another mod has replaced the chest
 * renderer, and switches itself off if anything fails. Render thread.
 */
public final class GpuChests implements ChestInstancing.World {
	/** What the block entity mixin adds to every block entity. */
	public interface Slot {
		Object arctic$gpu();

		void arctic$gpu(Object value);

		/** The last lid event told it to be open. */
		boolean arctic$lidOpen();

		void arctic$lidOpen(boolean open);
	}

	private static final Logger LOG = LoggerFactory.getLogger("Arctic");
	/** Marks a block entity that is never drawn this way (not one of the game's chests). */
	private static final Object NEVER = new Object();
	private static final boolean OFF = "false".equals(System.getProperty("arctic.gpuChests"));
	/** Mods that draw block entities their own way, or again from another view. */
	private static final String[] CONFLICTS = {"iris", "oculus", "optifabric", "immersive_portals", "imm_ptl_core", "entity_model_features",
			"entity_texture_features", "enhancedblockentities", "fastchest", "canvas", "vulkanmod"};
	private static final Identifier ENTITY_SHADER = Identifier.withDefaultNamespace("core/entity");
	private static final Identifier SHADER = Identifier.fromNamespaceAndPath("arctic", "core/chest_instanced");
	private static final VertexFormat FORMAT = VertexFormat.builder(0)
			.addAttribute("Position", GpuFormat.RGB32_FLOAT)
			.addAttribute("UV0", GpuFormat.RG32_FLOAT)
			.addAttribute("UV1", GpuFormat.RG16_SINT)
			.addAttribute("Normal", GpuFormat.RGB32_FLOAT)
			.build();
	private static final BindGroupLayout LAYOUT = BindGroupLayout.builder()
			.withUniform("ArcticChestDraw", UniformType.UNIFORM_BUFFER)
			.withUniform("ArcticChests", UniformType.TEXEL_BUFFER, GpuFormat.RGBA32_FLOAT)
			.build();
	private static final ChestType[] TYPES = {ChestType.SINGLE, ChestType.LEFT, ChestType.RIGHT};
	private static final ChestRenderState.ChestMaterialType[] MATERIALS = ChestRenderState.ChestMaterialType.values();
	/** Bytes of the per-draw uniform block (four vec4). */
	private static final int DRAW_BYTES = 64;
	private static final int RING = 3;

	/** The part added to the game's entity vertex shader: its inputs, worked out per chest. */
	private static final String PART = """
			// Arctic: chests drawn instanced. Vertices arrive in the chest model's own space (lid shut);
			// each chest's place, facing, light, lid and texture come from ArcticChests.
			layout(std140) uniform ArcticChestDraw {
			    ivec4 ArcticBase;   // x: this draw's first texel; y: the texture table's first texel
			    vec4 ArcticOrigin;  // xyz: the anchor from the camera's block (whole blocks); w: draw distance, squared
			    vec4 ArcticCamera;  // xyz: the camera inside its block
			    vec4 ArcticHinge;   // xyz: the lid's hinge
			};

			uniform samplerBuffer ArcticChests;

			in vec3 Position;
			in vec2 UV0;
			in ivec2 UV1; // x: 1 on the lid and the lock
			in vec3 Normal;

			// cos, sin of the model's turn about the vertical, per facing (quarter turns).
			const vec2 ARCTIC_TURN[4] = vec2[4](vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(-1.0, 0.0), vec2(0.0, 1.0));

			void arctic_main() {
			    int at = ArcticBase.x + gl_InstanceID * 2;
			    vec4 place = texelFetch(ArcticChests, at);
			    vec4 look = texelFetch(ArcticChests, at + 1);
			    vec4 sprite = texelFetch(ArcticChests, ArcticBase.y + int(look.w));

			    vec3 p = Position;
			    vec3 n = Normal;
			    if (UV1.x != 0 && look.z != 0.0) {
			        float angle = -look.z * 1.5707964;
			        float c = cos(angle);
			        float s = sin(angle);
			        vec3 d = p - ArcticHinge.xyz;
			        p = ArcticHinge.xyz + vec3(d.x, d.y * c - d.z * s, d.y * s + d.z * c);
			        n = vec3(n.x, n.y * c - n.z * s, n.y * s + n.z * c);
			    }
			    vec2 turn = ARCTIC_TURN[int(place.w) & 3];
			    vec3 d = p - vec3(0.5, 0.0, 0.5);
			    p = vec3(0.5 + d.x * turn.x + d.z * turn.y, p.y, 0.5 - d.x * turn.y + d.z * turn.x);
			    n = vec3(n.x * turn.x + n.z * turn.y, n.y, -n.x * turn.y + n.z * turn.x);

			    vec3 block = place.xyz + ArcticOrigin.xyz;
			    vec3 centre = block + (vec3(0.5) - ArcticCamera.xyz);

			    minecraft_Position = block + (p - ArcticCamera.xyz);
			    minecraft_Normal = n;
			    minecraft_Color = vec4(1.0);
			    minecraft_UV0 = sprite.xy + UV0 * sprite.zw;
			    minecraft_UV1 = ivec2(0, 10);
			    minecraft_UV2 = ivec2(look.xy);
			    minecraft_main();
			    // As the game: a chest is drawn while its block's middle is closer than the draw distance.
			    if (dot(centre, centre) >= ArcticOrigin.w) {
			        gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
			    }
			}
			""";
	private static final String HEADER = """
			vec3 minecraft_Position;
			vec4 minecraft_Color;
			vec2 minecraft_UV0;
			ivec2 minecraft_UV1;
			ivec2 minecraft_UV2;
			vec3 minecraft_Normal;
			void minecraft_main();
			""";
	private static final String INPUTS = "Position|Color|UV0|UV1|UV2|Normal";
	private static final Pattern VERSION = Pattern.compile("(?m)^#version[^\\n]*\\n");
	private static final Pattern INPUT_DECLARATION = Pattern.compile(
			"(?m)^[ \\t]*(?:layout[ \\t]*\\([^)]*\\)[ \\t]*)?in[ \\t]+\\w+[ \\t]+(?:" + INPUTS + ")[ \\t]*;[^\\n]*$");
	private static final Pattern INPUT_USE = Pattern.compile("\\b(" + INPUTS + ")\\b");
	private static final Pattern MAIN = Pattern.compile("\\bvoid\\s+main\\s*\\(\\s*\\)");

	private static final GpuChests INSTANCE = new GpuChests();

	private final ChestInstancing chests = new ChestInstancing();
	private final boolean conflict;
	private boolean failed;
	/** Inside the world's extraction, with the path on. */
	private boolean inFrame;
	private boolean frameBegun;
	private float partialTick;
	private ClientLevel level;
	private ChestInstancing.Frame frame;
	/** A frame's instances are ready; the world's main pass hasn't been set up yet. */
	private boolean pending;
	/** The world's main pass is set up: draw at its solid features. */
	private boolean armed;

	// On the GPU; made again after every resource reload.
	private boolean ready;
	private final GpuBuffer[] meshes = new GpuBuffer[ChestInstancing.KINDS];
	private final int[] meshVertices = new int[ChestInstancing.KINDS];
	private final Vector3f hinge = new Vector3f();
	private RenderType renderType;
	private RenderPipeline pipeline;
	private boolean christmas;
	private final GpuBuffer[] instanceBuffers = new GpuBuffer[RING];
	private final GpuBuffer[] drawBuffers = new GpuBuffer[RING];
	private int instanceSlot;
	private int drawSlot;
	/** The frame's instances aren't on the GPU yet. */
	private boolean needUpload = true;
	private ByteBuffer scratch;
	private long lastReport;
	private static Constructor<RenderPipeline> pipelineConstructor;

	private GpuChests() {
		String found = null;
		for (String id : CONFLICTS) {
			if (FabricLoader.getInstance().isModLoaded(id)) {
				found = id;
				break;
			}
		}
		conflict = found != null;
		if (found != null) {
			LOG.info("Arctic: {} is installed; chests stay on the game's renderer", found);
		}
	}

	private boolean on() {
		ClientConfig config = ArcticClient.config();
		return !OFF && !conflict && !failed && config != null && config.blockEntityCulling && config.chestInstancing;
	}

	// ---- the frame ----

	/** The world's extraction starts. */
	public static void begin() {
		GpuChests g = INSTANCE;
		g.inFrame = false;
		g.frameBegun = false;
		g.pending = false;
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft.level != g.level) {
			// Another world: nothing kept from the last one (also when the option is off, so nothing of it stays in memory).
			g.chests.clear();
			g.level = minecraft.level;
		}
		if (g.level == null || !g.on()) {
			return;
		}
		try {
			g.prepare(minecraft);
			g.inFrame = true;
		} catch (RuntimeException | LinkageError | ReflectiveOperationException e) {
			g.fail("couldn't set up", e);
		}
	}

	/**
	 * The game is about to make this block entity's render state. True: it is a chest drawn from here
	 * this frame, and the game makes nothing for it.
	 */
	public static boolean capture(BlockEntity entity, float partialTick, Object breaking, boolean global) {
		GpuChests g = INSTANCE;
		if (!g.inFrame || global || entity == null) {
			return false;
		}
		Object slot = ((Slot) entity).arctic$gpu();
		if (slot == NEVER) {
			return false;
		}
		ChestInstancing.Entry entry = slot instanceof ChestInstancing.Entry kept && kept.live() ? kept : g.register(entity);
		if (entry == null) {
			return false;
		}
		if (!g.frameBegun) {
			g.frameBegun = true;
			g.partialTick = partialTick;
			Vec3 camera = Minecraft.getInstance().gameRenderer.mainCamera().position();
			g.chests.beginFrame(camera.x, camera.y, camera.z, BlockEntityCulling.distance(ArcticClient.config()));
		}
		return g.chests.see(entry, breaking != null);
	}

	/** The world's extraction is over: lay the frame's chests out. */
	public static void end() {
		GpuChests g = INSTANCE;
		if (!g.inFrame) {
			return;
		}
		g.inFrame = false;
		if (!g.frameBegun) {
			return;
		}
		try {
			g.frame = g.chests.endFrame(g, g.partialTick);
			g.needUpload |= g.frame.changed;
			g.pending = g.frame.total() > 0;
		} catch (RuntimeException | LinkageError e) {
			g.fail("couldn't lay out a frame", e);
		}
	}

	/** The world's main pass is being set up (it runs after this, once). */
	public static void mainPass() {
		GpuChests g = INSTANCE;
		g.armed = g.pending;
		g.pending = false;
	}

	/** The world's solid features are about to be drawn, after the solid terrain: the chests go here. */
	public static void draw() {
		GpuChests g = INSTANCE;
		if (!g.armed) {
			return;
		}
		g.armed = false;
		try {
			g.drawFrame();
		} catch (RuntimeException | LinkageError e) {
			g.fail("couldn't draw", e);
		}
	}

	private void fail(String what, Throwable e) {
		if (!failed) {
			failed = true;
			LOG.error("Arctic: {} chests on the GPU; they are drawn the game's way from now on", what, e);
		}
		inFrame = false;
		pending = false;
		armed = false;
	}

	// ---- which chests ----

	private ChestInstancing.Entry register(BlockEntity entity) {
		Class<?> type = entity.getClass();
		if (type != ChestBlockEntity.class && type != TrappedChestBlockEntity.class && type != EnderChestBlockEntity.class) {
			((Slot) entity).arctic$gpu(NEVER);
			return null;
		}
		Object renderer = Minecraft.getInstance().getBlockEntityRenderDispatcher().getRenderer(entity);
		if (renderer == null || renderer.getClass() != ChestRenderer.class || entity.getLevel() != level || entity.isRemoved()
				|| !entity.getType().isValid(entity.getBlockState())) {
			return null;
		}
		BlockPos pos = entity.getBlockPos();
		ChestInstancing.Entry entry = chests.add(entity, pos.getX(), pos.getY(), pos.getZ(), !shut(entity));
		((Slot) entity).arctic$gpu(entry);
		return entry;
	}

	/** The block entity was removed or its block changed: it is looked at afresh when next drawn. */
	public static void gone(BlockEntity entity) {
		Slot slot = (Slot) entity;
		if (slot.arctic$gpu() instanceof ChestInstancing.Entry entry) {
			INSTANCE.chests.remove(entry);
			slot.arctic$gpu(null);
		}
	}

	/** A chest's lid was told to open or shut (the game's block event 1). */
	public static void lidEvent(BlockEntity entity, int event, int count) {
		if (event != 1 || entity.getLevel() == null || !entity.getLevel().isClientSide()) {
			return;
		}
		Slot slot = (Slot) entity;
		slot.arctic$lidOpen(count > 0);
		wake(slot);
		BlockEntity other = otherHalf(entity);
		if (other != null) {
			wake((Slot) other);
		}
	}

	private static void wake(Slot slot) {
		if (slot.arctic$gpu() instanceof ChestInstancing.Entry entry) {
			INSTANCE.chests.wake(entry);
		}
	}

	/** Light changed in this section. */
	public static void lightChanged(int sx, int sy, int sz) {
		INSTANCE.chests.markDirtyAround(sx, sy, sz);
	}

	/** Resources were reloaded: models, textures and shaders are taken again. */
	public static void reload() {
		INSTANCE.ready = false;
	}

	private static BlockEntity otherHalf(BlockEntity entity) {
		BlockState state = entity.getBlockState();
		if (!(state.getBlock() instanceof ChestBlock) || !state.hasProperty(ChestBlock.TYPE) || state.getValue(ChestBlock.TYPE) == ChestType.SINGLE
				|| entity.getLevel() == null) {
			return null;
		}
		return entity.getLevel().getBlockEntity(ChestBlock.getConnectedBlockPos(entity.getBlockPos(), state));
	}

	/** The lid as the game's renderer reads it: a double chest's halves move together. */
	private static float openness(BlockEntity entity, float partialTick) {
		BlockState state = entity.getBlockState();
		DoubleBlockCombiner.NeighborCombineResult<? extends ChestBlockEntity> combined;
		if (entity.getLevel() != null && state.getBlock() instanceof ChestBlock chest) {
			combined = chest.combine(state, entity.getLevel(), entity.getBlockPos(), true);
		} else {
			combined = DoubleBlockCombiner.Combiner::acceptNone;
		}
		return combined.apply(ChestBlock.opennessCombiner((LidBlockEntity) entity)).get(partialTick);
	}

	private static boolean shut(BlockEntity entity) {
		if (((Slot) entity).arctic$lidOpen()) {
			return false;
		}
		BlockEntity other = otherHalf(entity);
		if (other != null && ((Slot) other).arctic$lidOpen()) {
			return false;
		}
		return openness(entity, 0f) == 0f && openness(entity, 1f) == 0f;
	}

	@Override
	public boolean describe(ChestInstancing.Entry entry) {
		BlockEntity entity = (BlockEntity) entry.owner;
		if (entity.isRemoved() || entity.getLevel() != level || level == null) {
			return false;
		}
		BlockState state = entity.getBlockState();
		if (!entity.getType().isValid(state) || !state.hasProperty(ChestBlock.FACING)) {
			return false;
		}
		ChestType type = state.hasProperty(ChestBlock.TYPE) ? state.getValue(ChestBlock.TYPE) : ChestType.SINGLE;
		Direction facing = state.getValue(ChestBlock.FACING);
		entry.kind = type == ChestType.SINGLE ? 0 : type == ChestType.LEFT ? 1 : 2;
		entry.facing = Math.round(facing.toYRot() / 90f) & 3;
		entry.sprite = material(entity, state).ordinal() * ChestInstancing.KINDS + entry.kind;
		BlockPos pos = entity.getBlockPos();
		int light = LightCoordsUtil.getLightCoords(level, pos);
		if (type != ChestType.SINGLE && state.getBlock() instanceof ChestBlock chest) {
			light = chest.combine(state, level, pos, true).apply(new BrightnessCombiner<>()).applyAsInt(light);
		}
		entry.lightU = light & 0xFFFF;
		entry.lightV = light >>> 16;
		return true;
	}

	/** As ChestRenderer.getChestMaterial. */
	private ChestRenderState.ChestMaterialType material(BlockEntity entity, BlockState state) {
		if (state.getBlock() instanceof CopperChestBlock copper) {
			return switch (copper.getState()) {
				case UNAFFECTED -> ChestRenderState.ChestMaterialType.COPPER_UNAFFECTED;
				case EXPOSED -> ChestRenderState.ChestMaterialType.COPPER_EXPOSED;
				case WEATHERED -> ChestRenderState.ChestMaterialType.COPPER_WEATHERED;
				case OXIDIZED -> ChestRenderState.ChestMaterialType.COPPER_OXIDIZED;
			};
		}
		if (entity instanceof EnderChestBlockEntity) {
			return ChestRenderState.ChestMaterialType.ENDER_CHEST;
		}
		if (christmas) {
			return ChestRenderState.ChestMaterialType.CHRISTMAS;
		}
		return entity instanceof TrappedChestBlockEntity ? ChestRenderState.ChestMaterialType.TRAPPED : ChestRenderState.ChestMaterialType.REGULAR;
	}

	@Override
	public float open(ChestInstancing.Entry entry, float partialTick) {
		// As ChestRenderer.submit eases it.
		float open = 1f - openness((BlockEntity) entry.owner, partialTick);
		return 1f - open * open * open;
	}

	@Override
	public boolean shut(ChestInstancing.Entry entry) {
		return shut((BlockEntity) entry.owner);
	}

	@Override
	public void dropped(ChestInstancing.Entry entry) {
		((Slot) entry.owner).arctic$gpu(null);
	}

	// ---- on the GPU ----

	/** Models, textures and the pipeline, made after a reload; the pipeline is checked every frame (a cleared cache compiles it again from here). */
	private void prepare(Minecraft minecraft) throws ReflectiveOperationException {
		if (!ready) {
			build(minecraft);
			ready = true;
		}
		ShaderSource source = (id, type) -> type == ShaderType.VERTEX && SHADER.equals(id)
				? vertexSource(minecraft.getShaderManager().getShader(ENTITY_SHADER, ShaderType.VERTEX))
				: minecraft.getShaderManager().getShader(id, type);
		if (!RenderSystem.getDevice().precompilePipeline(pipeline, source).isValid()) {
			throw new IllegalStateException("the GPU couldn't compile the instanced chest shader");
		}
	}

	private void build(Minecraft minecraft) throws ReflectiveOperationException {
		for (int kind = 0; kind < ChestInstancing.KINDS; kind++) {
			if (meshes[kind] != null) {
				meshes[kind].close();
				meshes[kind] = null;
			}
		}
		christmas = ChestRenderer.xmasTextures();
		ChestModel model = null;
		for (int kind = 0; kind < ChestInstancing.KINDS; kind++) {
			ModelPart root = minecraft.getEntityModels().bakeLayer(ChestRenderer.LAYERS.select(TYPES[kind]));
			model = new ChestModel(root);
			mesh(kind, root);
		}
		if (MATERIALS.length * ChestInstancing.KINDS > ChestInstancing.SPRITES) {
			throw new IllegalStateException("more chest textures than the table holds");
		}
		RenderType type = null;
		for (ChestRenderState.ChestMaterialType material : MATERIALS) {
			for (int kind = 0; kind < ChestInstancing.KINDS; kind++) {
				SpriteId id = Sheets.chooseSprite(material, TYPES[kind]);
				RenderType spriteType = id.renderType(model.renderType());
				if (type == null) {
					type = spriteType;
				} else if (type != spriteType) {
					throw new IllegalStateException("chest textures aren't in one atlas");
				}
				TextureAtlasSprite sprite = minecraft.getAtlasManager().get(id);
				chests.sprite(material.ordinal() * ChestInstancing.KINDS + kind, sprite.getU0(), sprite.getV0(), sprite.getU1() - sprite.getU0(),
						sprite.getV1() - sprite.getV0());
			}
		}
		RenderPipeline original = type.pipeline();
		if (!ENTITY_SHADER.equals(original.getVertexShader()) || original.getPrimitiveTopology() != PrimitiveTopology.QUADS
				|| !DefaultVertexFormat.ENTITY.equals(original.getVertexFormatBinding(0)) || type.hasBlending()) {
			throw new IllegalStateException("the chest render type isn't the game's own: " + type);
		}
		renderType = type;
		pipeline = twin(original);
		chests.markAllDirty();
		needUpload = true;
		LOG.info("Arctic: chests are drawn on the GPU ({} + {} + {} vertices a model)", meshVertices[0], meshVertices[1], meshVertices[2]);
	}

	/** A chest model's faces, lid shut, in the block's space; each vertex says whether it moves with the lid. */
	private void mesh(int kind, ModelPart root) {
		ModelPart lid = root.getChild("lid");
		ModelPart lock = root.getChild("lock");
		if (lid.x != lock.x || lid.y != lock.y || lid.z != lock.z) {
			throw new IllegalStateException("the chest's lid and lock don't share a hinge");
		}
		if (kind == 0) {
			hinge.set(lid.x / 16f, lid.y / 16f, lid.z / 16f);
		} else if (hinge.x != lid.x / 16f || hinge.y != lid.y / 16f || hinge.z != lid.z / 16f) {
			throw new IllegalStateException("the chest models' hinges differ");
		}
		List<float[]> vertices = new ArrayList<>();
		root.visit(new PoseStack(), (pose, path, index, cube) -> {
			float moves = path.endsWith("/lid") || path.endsWith("/lock") ? 1f : 0f;
			for (ModelPart.Polygon polygon : cube.polygons) {
				Vector3f normal = pose.transformNormal(polygon.normal().x(), polygon.normal().y(), polygon.normal().z(), new Vector3f());
				for (ModelPart.Vertex vertex : polygon.vertices()) {
					Vector3f at = pose.pose().transformPosition(vertex.worldX(), vertex.worldY(), vertex.worldZ(), new Vector3f());
					vertices.add(new float[] {at.x, at.y, at.z, vertex.u(), vertex.v(), moves, normal.x, normal.y, normal.z});
				}
			}
		});
		if (vertices.isEmpty() || vertices.size() % 4 != 0) {
			throw new IllegalStateException("the chest model has no faces");
		}
		ByteBuffer data = MemoryUtil.memAlloc(vertices.size() * FORMAT.getVertexSize()).order(ByteOrder.nativeOrder());
		try {
			for (float[] v : vertices) {
				data.putFloat(v[0]).putFloat(v[1]).putFloat(v[2]);
				data.putFloat(v[3]).putFloat(v[4]);
				data.putShort((short) v[5]).putShort((short) 0);
				data.putFloat(v[6]).putFloat(v[7]).putFloat(v[8]);
			}
			data.flip();
			meshes[kind] = RenderSystem.getDevice().createBuffer(() -> "Arctic chest model", GpuBuffer.USAGE_VERTEX, data);
			meshVertices[kind] = vertices.size();
		} finally {
			MemoryUtil.memFree(data);
		}
	}

	/** The chest pipeline with the instanced vertex shader, this vertex format and the instance data; all else the game's. */
	private static RenderPipeline twin(RenderPipeline original) throws ReflectiveOperationException {
		if (pipelineConstructor == null) {
			Constructor<RenderPipeline> found = RenderPipeline.class.getDeclaredConstructor(Identifier.class, Identifier.class, Identifier.class,
					ShaderDefines.class, List.class, ColorTargetState[].class, DepthStencilState.class, PolygonMode.class, boolean.class,
					VertexFormat[].class, PrimitiveTopology.class, int.class);
			found.setAccessible(true);
			pipelineConstructor = found;
		}
		List<BindGroupLayout> layouts = new ArrayList<>(original.getBindGroupLayouts());
		layouts.add(LAYOUT);
		VertexFormat[] bindings = original.getVertexFormatBindings().clone();
		java.util.Arrays.fill(bindings, null);
		bindings[0] = FORMAT;
		Identifier location = Identifier.fromNamespaceAndPath("arctic", "instanced/" + original.getLocation().getPath());
		return pipelineConstructor.newInstance(location, SHADER, original.getFragmentShader(), original.getShaderDefines(), List.copyOf(layouts),
				original.getColorTargetStates().clone(), original.getDepthStencilState(), original.getPolygonMode(), original.isCull(), bindings,
				original.getPrimitiveTopology(), original.getSortKey());
	}

	/**
	 * The game's entity vertex shader with its vertex inputs turned into plain variables and its main()
	 * renamed; {@link #PART} fills the variables in per chest and runs it.
	 */
	static String vertexSource(String game) {
		if (game == null) {
			throw new IllegalStateException("the game's entity shader isn't loaded");
		}
		Matcher version = VERSION.matcher(game);
		if (!version.find()) {
			throw new IllegalStateException("the entity shader has no #version line");
		}
		String body = INPUT_DECLARATION.matcher(game.substring(version.end())).replaceAll("");
		Matcher main = MAIN.matcher(body);
		if (!main.find()) {
			throw new IllegalStateException("the entity shader has no main()");
		}
		int at = main.start();
		if (main.find()) {
			throw new IllegalStateException("the entity shader has more than one main()");
		}
		body = body.substring(0, at) + "void minecraft_main()" + body.substring(at).replaceFirst(MAIN.pattern(), "");
		body = INPUT_USE.matcher(body).replaceAll("minecraft_$1");
		return game.substring(0, version.end()) + HEADER + body + "\n" + PART + "\nvoid main() {\n    arctic_main();\n}\n";
	}

	private void drawFrame() {
		ChestInstancing.Frame f = frame;
		if (f == null || f.total() == 0 || !ready) {
			return;
		}
		GpuDevice device = RenderSystem.getDevice();
		if (needUpload || instanceBuffers[instanceSlot] == null) {
			// A buffer the GPU may still be drawing last frame from is left alone: the next one in the ring is written.
			instanceSlot = (instanceSlot + 1) % RING;
			long bytes = (long) f.floats * 4;
			instanceBuffers[instanceSlot] = ensure(device, instanceBuffers[instanceSlot], bytes,
					GpuBuffer.USAGE_UNIFORM_TEXEL_BUFFER | GpuBuffer.USAGE_COPY_DST, "Arctic chest instances");
			ByteBuffer bytesOut = scratch((int) bytes);
			bytesOut.asFloatBuffer().put(f.data, 0, f.floats);
			bytesOut.limit((int) bytes);
			device.createCommandEncoder().writeToBuffer(instanceBuffers[instanceSlot].slice(0, bytes), bytesOut);
			needUpload = false;
		}
		GpuBuffer instances = instanceBuffers[instanceSlot];

		int alignment = Math.max(DRAW_BYTES, device.getDeviceInfo().limits().minUniformOffsetAlignment());
		alignment = (alignment + 15) / 16 * 16;
		int drawBytes = alignment * ChestInstancing.KINDS;
		drawSlot = (drawSlot + 1) % RING;
		drawBuffers[drawSlot] = ensure(device, drawBuffers[drawSlot], drawBytes, GpuBuffer.USAGE_UNIFORM | GpuBuffer.USAGE_COPY_DST,
				"Arctic chest draws");
		ByteBuffer draw = scratch(drawBytes);
		float distance = BlockEntityCulling.distance(ArcticClient.config());
		for (int kind = 0; kind < ChestInstancing.KINDS; kind++) {
			int at = kind * alignment;
			draw.putInt(at, f.first[kind]).putInt(at + 4, 0).putInt(at + 8, 0).putInt(at + 12, 0);
			draw.putFloat(at + 16, f.originX).putFloat(at + 20, f.originY).putFloat(at + 24, f.originZ).putFloat(at + 28, distance * distance);
			draw.putFloat(at + 32, f.fracX).putFloat(at + 36, f.fracY).putFloat(at + 40, f.fracZ).putFloat(at + 44, 0f);
			draw.putFloat(at + 48, hinge.x).putFloat(at + 52, hinge.y).putFloat(at + 56, hinge.z).putFloat(at + 60, 0f);
		}
		draw.limit(drawBytes);
		device.createCommandEncoder().writeToBuffer(drawBuffers[drawSlot].slice(0, drawBytes), draw);

		PreparedRenderType type = renderType.prepare();
		RenderTarget target = type.outputTarget().getRenderTarget();
		GpuTextureView color = RenderSystem.outputColorTextureOverride != null ? RenderSystem.outputColorTextureOverride : target.getColorTextureView();
		GpuTextureView depth = target.useDepth
				? (RenderSystem.outputDepthTextureOverride != null ? RenderSystem.outputDepthTextureOverride : target.getDepthTextureView())
				: null;
		RenderSystem.AutoStorageIndexBuffer indices = RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS);
		int draws = 0;
		try (RenderPass pass = device.createCommandEncoder().createRenderPass(() -> "Arctic instanced chests", color, Optional.empty(), depth,
				OptionalDouble.empty())) {
			pass.setPipeline(pipeline);
			if (type.scissorState().enabled()) {
				pass.enableScissor(type.scissorState().x(), type.scissorState().y(), type.scissorState().width(), type.scissorState().height());
			}
			RenderSystem.bindDefaultUniforms(pass);
			pass.setUniform("DynamicTransforms", type.dynamicTransforms());
			pass.setUniform("ArcticChests", instances);
			for (PreparedRenderType.Texture texture : type.textures()) {
				pass.bindTexture(texture.name(), texture.textureView(), texture.sampler());
			}
			for (int kind = 0; kind < ChestInstancing.KINDS; kind++) {
				if (f.count[kind] == 0) {
					continue;
				}
				int indexCount = meshVertices[kind] / 4 * 6;
				pass.setUniform("ArcticChestDraw", drawBuffers[drawSlot].slice((long) kind * alignment, DRAW_BYTES));
				pass.setVertexBuffer(0, meshes[kind].slice());
				pass.setIndexBuffer(indices.getBuffer(indexCount), indices.type());
				pass.drawIndexed(indexCount, f.count[kind], 0, 0, 0);
				draws++;
			}
		}
		report(f, draws);
	}

	/** Every ten seconds with -Darctic.gpuChests.report=true: how many chests took how many draws. */
	private void report(ChestInstancing.Frame f, int draws) {
		long now = System.nanoTime();
		if (now - lastReport > 10_000_000_000L && Boolean.getBoolean("arctic.gpuChests.report")) {
			lastReport = now;
			LOG.info("Arctic: {} chests in {} GPU draws this frame ({} kept, {} sections, {} uploads and {} section bakes since the last line)",
					f.total(), draws, chests.size(), chests.sectionCount(), chests.uploads, chests.bakes);
			chests.resetCounters();
		}
	}

	private ByteBuffer scratch(int bytes) {
		if (scratch == null || scratch.capacity() < bytes) {
			if (scratch != null) {
				MemoryUtil.memFree(scratch);
			}
			scratch = MemoryUtil.memAlloc(bytes + bytes / 2).order(ByteOrder.nativeOrder());
		}
		scratch.clear();
		return scratch;
	}

	private static GpuBuffer ensure(GpuDevice device, GpuBuffer buffer, long size, int usage, String label) {
		if (buffer != null && !buffer.isClosed() && buffer.size() >= size) {
			return buffer;
		}
		if (buffer != null) {
			buffer.close();
		}
		return device.createBuffer(() -> label, usage, Math.max(size + size / 2, 16 * 1024));
	}
}
//#endif
