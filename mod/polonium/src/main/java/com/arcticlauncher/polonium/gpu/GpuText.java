package com.arcticlauncher.polonium.gpu;

import com.arcticlauncher.polonium.GlyphRuns;
import com.mojang.blaze3d.GpuFormat;
import com.mojang.blaze3d.PrimitiveTopology;
import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.buffers.GpuBufferSlice;
import com.mojang.blaze3d.pipeline.BindGroupLayout;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.shaders.UniformType;
import com.mojang.blaze3d.systems.GpuDevice;
import com.mojang.blaze3d.systems.RenderPass;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.textures.GpuTextureView;
import com.mojang.blaze3d.vertex.VertexFormat;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.OptionalDouble;
import net.minecraft.client.gui.Font;
import net.minecraft.client.renderer.rendertype.PreparedRenderType;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.resources.Identifier;
import org.jspecify.annotations.Nullable;
import org.joml.Matrix4fc;
import org.lwjgl.system.MemoryUtil;

/**
 * Name tags on the GPU, kept there between frames. A tag's quads are recorded
 * once in its own space ({@link GlyphRuns}) and uploaded once into a glyph
 * pool on the GPU; each frame a tag costs only its pose and light, plus one
 * small record per run (where its glyphs are, how many, which tag). A render
 * type's tags are one instanced draw: each instance is one run, its vertices
 * read from the pool, and slots past a short run's end collapse to nothing.
 * Draws keep the order the game submitted them in, as the game merges a
 * render type's tags within a group.
 *
 * When the pool fills (tag texts keep changing) it grows, up to a limit, and
 * then starts over; runs simply upload again when next drawn.
 *
 * On by default; -Dpolonium.gpuText=false keeps name tags on the game's path.
 * One of these per NameTagFeatureRenderer; render thread only.
 */
public final class GpuText implements GpuFeature {
	/** Per vertex of the shared slot buffer: its number within a run. */
	static final VertexFormat SLOT_FORMAT = VertexFormat.builder(0)
			.addAttribute("Slot", GpuFormat.R32_SINT)
			.build();
	/** Per instance: the run's first pool vertex, its vertex count, its tag. */
	static final VertexFormat ITEM_FORMAT = VertexFormat.builder(1)
			.addAttribute("Item", GpuFormat.RGB32_SINT)
			.build();
	private static final int ITEM_BYTES = 12;
	/** The tags' data and the glyph pool. */
	static final BindGroupLayout LAYOUT = BindGroupLayout.builder()
			.withUniform("PoloniumInstances", UniformType.TEXEL_BUFFER, GpuFormat.RGBA32_FLOAT)
			.withUniform("PoloniumGlyphs", UniformType.TEXEL_BUFFER, GpuFormat.RGBA32_FLOAT)
			.build();
	/** Pose rows, then light. */
	private static final int TEXELS_PER_TAG = 4;
	/** Floats per pool vertex (two texels). */
	private static final int GLYPH_FLOATS = 8;
	private static final int FIRST_POOL_VERTICES = 64 * 1024;
	/** About 64 MB: past this the pool starts over instead of growing. */
	private static final int MAX_POOL_VERTICES = 2 * 1024 * 1024;
	static final Identifier TEXT_SHADER = Identifier.fromNamespaceAndPath("polonium", "core/text_pooled");
	static final Identifier BACKGROUND_SHADER = Identifier.fromNamespaceAndPath("polonium", "core/text_background_pooled");
	private static final Identifier GAME_TEXT = Identifier.withDefaultNamespace("core/text");
	private static final Identifier GAME_BACKGROUND = Identifier.withDefaultNamespace("core/text_background");
	private static final int FRAMES_IN_FLIGHT = 3;
	private static final boolean ENABLED = !"false".equals(System.getProperty("polonium.gpuText"));

	/** Whether name tags go to the GPU at all. */
	static boolean enabled() {
		return ENABLED;
	}

	private final Map<RenderType, PreparedRenderType> prepared = new IdentityHashMap<>();
	private final List<List<Batch>> groups = new ArrayList<>();
	private final List<Batch> spare = new ArrayList<>();
	private final InstanceData tags = new InstanceData();
	private @Nullable List<Batch> current;
	private boolean uploaded;
	private long frame;
	private long lastReport;
	private final GpuBuffer[] itemBuffers = new GpuBuffer[FRAMES_IN_FLIGHT];
	private final GpuBuffer[] tagBuffers = new GpuBuffer[FRAMES_IN_FLIGHT];

	// The glyph pool: runs' vertices, placed once and drawn from every frame. One
	// for every GpuText (the game has several name tag renderers), since a run
	// remembers a single place in it.
	private static @Nullable GpuBuffer pool;
	private static int poolCapacity;
	private static int poolUsed;
	/** Bumped whenever the pool starts over: runs placed before must be placed again. */
	private static int poolGeneration;
	/** Vertices placed but not uploaded yet: they're {@code [pendingFrom, poolUsed)}. */
	private static int pendingFrom;
	private static float[] pending = new float[GLYPH_FLOATS * 4096];
	/** The pool ran out: it grows (or starts over) at the end of the frame. */
	private static boolean poolFull;
	/** Vertices placed since the last report (for the log: steady tags place none). */
	private static long placedSinceReport;

	// Slot numbers 0..n-1, shared by every draw.
	private static @Nullable GpuBuffer slots;
	private static int slotCount;

	private static final class Batch {
		RenderType type;
		PreparedRenderType prepared;
		RenderPipeline pipeline;
		int[] items = new int[3 * 256];
		int count;
		int maxVertices;
		long offset;
	}

	@Override
	public void beginGroup(boolean strictlyOrdered) {
		List<Batch> group = new ArrayList<>();
		groups.add(group);
		current = group;
	}

	@Override
	public void endGroup() {
		current = null;
	}

	/** Take this tag onto the GPU; false to let the game write its vertices. */
	public boolean capture(Font.PreparedText text, Font.DisplayMode mode, Matrix4fc pose, int lightCoords) {
		List<Batch> group = current;
		if (!ENABLED || group == null || GpuBatches.disabled() || poolFull) {
			return false;
		}
		try {
			GlyphRuns.Run[] runs = GlyphRuns.runs(text, mode);
			for (GlyphRuns.Run run : runs) {
				if (shaderFor(run.renderType(mode)) == null || !place(run)) {
					return false;
				}
			}
			int tag = tags.texels() / TEXELS_PER_TAG;
			tags.put(pose.m00(), pose.m10(), pose.m20(), pose.m30());
			tags.put(pose.m01(), pose.m11(), pose.m21(), pose.m31());
			tags.put(pose.m02(), pose.m12(), pose.m22(), pose.m32());
			tags.put(lightCoords & 0xFFFF, (lightCoords >>> 16) & 0xFFFF, 0, 0);
			for (GlyphRuns.Run run : runs) {
				add(batchFor(group, run.renderType(mode)), run, tag);
			}
			return true;
		} catch (RuntimeException | LinkageError e) {
			GpuBatches.disable("couldn't take a name tag onto the GPU", e);
			return false;
		}
	}

	/** Make sure the run's vertices are in the pool; false if the pool is full this frame. */
	private static boolean place(GlyphRuns.Run run) {
		if (run.poolGeneration == poolGeneration && run.poolStart >= 0) {
			return true;
		}
		int count = run.vertexCount();
		if (pool == null || poolUsed + count > poolCapacity) {
			poolFull = true;
			return false;
		}
		int at = (poolUsed - pendingFrom) * GLYPH_FLOATS;
		if (at + count * GLYPH_FLOATS > pending.length) {
			pending = Arrays.copyOf(pending, Math.max(pending.length * 2, at + count * GLYPH_FLOATS));
		}
		run.writeGlyphs(pending, at);
		run.poolStart = poolUsed;
		run.poolGeneration = poolGeneration;
		poolUsed += count;
		placedSinceReport += count;
		return true;
	}

	private static @Nullable Identifier shaderFor(RenderType type) {
		RenderPipeline pipeline = type.pipeline();
		if (pipeline.getPrimitiveTopology() != PrimitiveTopology.QUADS) {
			return null;
		}
		if (GAME_TEXT.equals(pipeline.getVertexShader())) {
			return TEXT_SHADER;
		}
		if (GAME_BACKGROUND.equals(pipeline.getVertexShader())) {
			return BACKGROUND_SHADER;
		}
		return null;
	}

	private Batch batchFor(List<Batch> group, RenderType type) {
		for (Batch batch : group) {
			if (batch.type == type) {
				return batch;
			}
		}
		Batch batch = spare.isEmpty() ? new Batch() : spare.removeLast();
		batch.type = type;
		batch.prepared = prepared.computeIfAbsent(type, RenderType::prepare);
		batch.pipeline = InstancedPipelines.twin(batch.prepared.pipeline(), shaderFor(type), LAYOUT, SLOT_FORMAT, ITEM_FORMAT);
		batch.count = 0;
		batch.maxVertices = 0;
		group.add(batch);
		return batch;
	}

	private static void add(Batch batch, GlyphRuns.Run run, int tag) {
		int at = batch.count * 3;
		if (at + 3 > batch.items.length) {
			batch.items = Arrays.copyOf(batch.items, batch.items.length * 2);
		}
		batch.items[at] = run.poolStart;
		batch.items[at + 1] = run.vertexCount();
		batch.items[at + 2] = tag;
		batch.count++;
		batch.maxVertices = Math.max(batch.maxVertices, run.vertexCount());
	}

	@Override
	public void executeGroup(int groupIndex) {
		if (groupIndex < 0 || groupIndex >= groups.size() || groups.get(groupIndex).isEmpty()) {
			return;
		}
		try {
			if (!uploaded) {
				upload();
				uploaded = true;
			}
			draw(groups.get(groupIndex));
		} catch (RuntimeException e) {
			GpuBatches.disable("drawing name tags on the GPU failed", e);
		}
	}

	private void upload() {
		GpuDevice device = RenderSystem.getDevice();
		int slot = (int) (frame % FRAMES_IN_FLIGHT);
		flushPool(device);
		long itemBytes = 0;
		int maxVertices = 0;
		for (List<Batch> group : groups) {
			for (Batch batch : group) {
				batch.offset = itemBytes;
				itemBytes += (long) batch.count * ITEM_BYTES;
				maxVertices = Math.max(maxVertices, batch.maxVertices);
			}
		}
		if (itemBytes == 0) {
			return;
		}
		ensureSlots(device, maxVertices);
		itemBuffers[slot] = ensure(device, itemBuffers[slot], itemBytes, GpuBuffer.USAGE_VERTEX | GpuBuffer.USAGE_MAP_WRITE, "Polonium name tag runs");
		long tagBytes = (long) tags.texels() * 16;
		tagBuffers[slot] = ensure(device, tagBuffers[slot], tagBytes,
				GpuBuffer.USAGE_UNIFORM_TEXEL_BUFFER | GpuBuffer.USAGE_MAP_WRITE, "Polonium name tags");
		try (GpuBufferSlice.MappedView view = itemBuffers[slot].slice(0, itemBytes).map(false, true)) {
			java.nio.IntBuffer out = view.data().order(ByteOrder.nativeOrder()).asIntBuffer();
			for (List<Batch> group : groups) {
				for (Batch batch : group) {
					out.put(batch.items, 0, batch.count * 3);
				}
			}
		}
		try (GpuBufferSlice.MappedView view = tagBuffers[slot].slice(0, tagBytes).map(false, true)) {
			tags.writeTo(view.data().order(ByteOrder.nativeOrder()));
		}
	}

	/** Upload what was placed since the last flush: one contiguous stretch at the end of the pool. */
	private static void flushPool(GpuDevice device) {
		int newVertices = poolUsed - pendingFrom;
		if (newVertices > 0 && pool != null) {
			int floats = newVertices * GLYPH_FLOATS;
			ByteBuffer data = MemoryUtil.memAlloc(floats * 4).order(ByteOrder.nativeOrder());
			try {
				data.asFloatBuffer().put(pending, 0, floats);
				device.createCommandEncoder().writeToBuffer(pool.slice((long) pendingFrom * GLYPH_FLOATS * 4, (long) floats * 4), data);
			} finally {
				MemoryUtil.memFree(data);
			}
		}
		pendingFrom = poolUsed;
	}

	/** The slot buffer covers the longest run. */
	private static void ensureSlots(GpuDevice device, int vertices) {
		if (slots != null && !slots.isClosed() && slotCount >= vertices) {
			return;
		}
		if (slots != null) {
			slots.close();
		}
		int count = Math.max(1024, Integer.highestOneBit(Math.max(1, vertices - 1)) << 1);
		ByteBuffer data = MemoryUtil.memAlloc(count * 4).order(ByteOrder.nativeOrder());
		try {
			for (int i = 0; i < count; i++) {
				data.putInt(i);
			}
			data.flip();
			slots = device.createBuffer(() -> "Polonium name tag slots", GpuBuffer.USAGE_VERTEX, data);
			slotCount = count;
		} finally {
			MemoryUtil.memFree(data);
		}
	}

	/** A pool of {@code vertices}; runs placed before (in another pool) place themselves again. */
	private static void newPool(int vertices) {
		if (pool != null) {
			pool.close();
		}
		pool = RenderSystem.getDevice().createBuffer(() -> "Polonium name tag glyphs",
				GpuBuffer.USAGE_UNIFORM_TEXEL_BUFFER | GpuBuffer.USAGE_COPY_DST, (long) vertices * GLYPH_FLOATS * 4);
		poolCapacity = vertices;
		poolUsed = 0;
		pendingFrom = 0;
		poolGeneration++;
	}

	private static GpuBuffer ensure(GpuDevice device, @Nullable GpuBuffer buffer, long size, int usage, String label) {
		if (buffer != null && !buffer.isClosed() && buffer.size() >= size) {
			return buffer;
		}
		if (buffer != null) {
			buffer.close();
		}
		return device.createBuffer(() -> label, usage, Math.max(size + size / 2, 64 * 1024));
	}

	private void draw(List<Batch> batches) {
		int slot = (int) (frame % FRAMES_IN_FLIGHT);
		GpuBuffer items = itemBuffers[slot];
		GpuBuffer tagData = tagBuffers[slot];
		if (items == null || tagData == null || pool == null || slots == null) {
			return;
		}
		RenderSystem.AutoStorageIndexBuffer indices = RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS);
		for (Batch batch : batches) {
			if (batch.count == 0) {
				continue;
			}
			PreparedRenderType type = batch.prepared;
			RenderTarget target = type.outputTarget().getRenderTarget();
			GpuTextureView color = RenderSystem.outputColorTextureOverride != null
					? RenderSystem.outputColorTextureOverride : target.getColorTextureView();
			GpuTextureView depth = target.useDepth
					? (RenderSystem.outputDepthTextureOverride != null ? RenderSystem.outputDepthTextureOverride : target.getDepthTextureView())
					: null;
			int indexCount = (batch.maxVertices + 3) / 4 * 6;
			try (RenderPass pass = RenderSystem.getDevice().createCommandEncoder()
					.createRenderPass(() -> "Polonium name tags", color, Optional.empty(), depth, OptionalDouble.empty())) {
				pass.setPipeline(batch.pipeline);
				if (type.scissorState().enabled()) {
					pass.enableScissor(type.scissorState().x(), type.scissorState().y(), type.scissorState().width(), type.scissorState().height());
				}
				RenderSystem.bindDefaultUniforms(pass);
				pass.setUniform("DynamicTransforms", type.dynamicTransforms());
				pass.setUniform("PoloniumInstances", tagData);
				pass.setUniform("PoloniumGlyphs", pool);
				pass.setVertexBuffer(0, slots.slice());
				pass.setVertexBuffer(1, items.slice(batch.offset, (long) batch.count * ITEM_BYTES));
				for (PreparedRenderType.Texture texture : type.textures()) {
					pass.bindTexture(texture.name(), texture.textureView(), texture.sampler());
				}
				pass.setIndexBuffer(indices.getBuffer(indexCount), indices.type());
				pass.drawIndexed(indexCount, batch.count, 0, 0, 0);
			}
		}
	}

	@Override
	public void endFrame() {
		int draws = 0;
		for (List<Batch> group : groups) {
			draws += group.size();
			spare.addAll(group);
		}
		long now = System.nanoTime();
		if (tags.texels() > 0 && now - lastReport > 10_000_000_000L) {
			lastReport = now;
			org.slf4j.LoggerFactory.getLogger("Polonium").info(
					"Polonium: {} name tags in {} GPU draws this frame ({} glyph vertices kept, {} placed in the last 10 s)",
					tags.texels() / TEXELS_PER_TAG, draws, poolUsed, placedSinceReport);
			placedSinceReport = 0;
		}
		if (pool == null) {
			newPool(FIRST_POOL_VERTICES);
		} else if (poolFull) {
			// Grow while there's room; past that, start over (tags upload again as they're drawn).
			newPool(Math.min(poolCapacity * 2, MAX_POOL_VERTICES));
		}
		poolFull = false;
		groups.clear();
		prepared.clear();
		tags.clear();
		current = null;
		uploaded = false;
		frame++;
	}
}
