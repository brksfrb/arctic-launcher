package com.arcticlauncher.polonium.gpu;

import com.arcticlauncher.polonium.GlyphRuns;
import com.mojang.blaze3d.GpuFormat;
import com.mojang.blaze3d.PrimitiveTopology;
import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.buffers.GpuBufferSlice;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.pipeline.RenderTarget;
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

/**
 * Name tags on the GPU. A tag's quads are recorded once in its own space
 * ({@link GlyphRuns}) and packed; each frame they're copied into one vertex
 * stream per render type, each vertex stamped with its tag's number, and the
 * GPU places them from the tags' poses. A render type's tags are one draw,
 * in the order the game submitted them (as the game merges them when its
 * groups may be reordered).
 *
 * One of these per NameTagFeatureRenderer; render thread only.
 */
public final class GpuText implements GpuFeature {
	/** Tag-space position, color, texture coordinates, and the tag's number. */
	static final VertexFormat FORMAT = VertexFormat.builder(0)
			.addAttribute("Position", GpuFormat.RGB32_FLOAT)
			.addAttribute("Color", GpuFormat.RGBA8_UNORM)
			.addAttribute("UV0", GpuFormat.RG32_FLOAT)
			.addAttribute("Instance", GpuFormat.R32_SINT)
			.build();
	static final int STRIDE = 28;
	private static final int INSTANCE_OFFSET = 24;
	/** Pose rows, then light. */
	private static final int TEXELS_PER_TAG = 4;
	static final Identifier TEXT_SHADER = Identifier.fromNamespaceAndPath("polonium", "core/text_instanced");
	static final Identifier BACKGROUND_SHADER = Identifier.fromNamespaceAndPath("polonium", "core/text_background_instanced");
	private static final Identifier GAME_TEXT = Identifier.withDefaultNamespace("core/text");
	private static final Identifier GAME_BACKGROUND = Identifier.withDefaultNamespace("core/text_background");
	private static final int FRAMES_IN_FLIGHT = 3;
	/**
	 * Off unless -Dpolonium.gpuText=true: copying every tag's quads into the
	 * stream each frame costs about what the game's own (Sodium-sped) vertex
	 * writes do, so it doesn't pay yet; keeping tags on the GPU between frames
	 * would.
	 */
	private static final boolean ENABLED = Boolean.getBoolean("polonium.gpuText");

	private final Map<RenderType, PreparedRenderType> prepared = new IdentityHashMap<>();
	private final List<List<Batch>> groups = new ArrayList<>();
	private final List<Batch> spare = new ArrayList<>();
	private final InstanceData tags = new InstanceData();
	private @Nullable List<Batch> current;
	private boolean uploaded;
	private long frame;
	private long lastReport;
	private final GpuBuffer[] vertexBuffers = new GpuBuffer[FRAMES_IN_FLIGHT];
	private final GpuBuffer[] tagBuffers = new GpuBuffer[FRAMES_IN_FLIGHT];

	private static final class Batch {
		RenderType type;
		PreparedRenderType prepared;
		RenderPipeline pipeline;
		byte[] bytes = new byte[STRIDE * 256];
		int size;
		long offset;

		int vertexCount() {
			return size / STRIDE;
		}
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
		if (!ENABLED || group == null || GpuBatches.disabled()) {
			return false;
		}
		try {
			GlyphRuns.Run[] runs = GlyphRuns.runs(text, mode);
			for (GlyphRuns.Run run : runs) {
				if (shaderFor(run.renderType(mode)) == null) {
					return false;
				}
			}
			int tag = tags.texels() / TEXELS_PER_TAG;
			tags.put(pose.m00(), pose.m10(), pose.m20(), pose.m30());
			tags.put(pose.m01(), pose.m11(), pose.m21(), pose.m31());
			tags.put(pose.m02(), pose.m12(), pose.m22(), pose.m32());
			tags.put(lightCoords & 0xFFFF, (lightCoords >>> 16) & 0xFFFF, 0, 0);
			for (GlyphRuns.Run run : runs) {
				append(batchFor(group, run.renderType(mode)), run.packed(), tag);
			}
			return true;
		} catch (RuntimeException | LinkageError e) {
			GpuBatches.disable("couldn't take a name tag onto the GPU", e);
			return false;
		}
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
		batch.pipeline = InstancedPipelines.twin(batch.prepared.pipeline(), shaderFor(type), FORMAT, InstancedPipelines.ENTITY_DATA);
		batch.size = 0;
		group.add(batch);
		return batch;
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
		long bytes = 0;
		for (List<Batch> group : groups) {
			for (Batch batch : group) {
				batch.offset = bytes;
				bytes += batch.size;
			}
		}
		if (bytes == 0) {
			return;
		}
		GpuDevice device = RenderSystem.getDevice();
		int slot = (int) (frame % FRAMES_IN_FLIGHT);
		vertexBuffers[slot] = ensure(device, vertexBuffers[slot], bytes, GpuBuffer.USAGE_VERTEX | GpuBuffer.USAGE_MAP_WRITE, "Polonium name tag vertices");
		long tagBytes = (long) tags.texels() * 16;
		tagBuffers[slot] = ensure(device, tagBuffers[slot], tagBytes,
				GpuBuffer.USAGE_UNIFORM_TEXEL_BUFFER | GpuBuffer.USAGE_MAP_WRITE, "Polonium name tags");
		try (GpuBufferSlice.MappedView view = vertexBuffers[slot].slice(0, bytes).map(false, true)) {
			ByteBuffer out = view.data();
			for (List<Batch> group : groups) {
				for (Batch batch : group) {
					out.put(batch.bytes, 0, batch.size);
				}
			}
		}
		try (GpuBufferSlice.MappedView view = tagBuffers[slot].slice(0, tagBytes).map(false, true)) {
			tags.writeTo(view.data().order(ByteOrder.nativeOrder()));
		}
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
		GpuBuffer vertices = vertexBuffers[slot];
		GpuBuffer tagData = tagBuffers[slot];
		RenderSystem.AutoStorageIndexBuffer indices = RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS);
		for (Batch batch : batches) {
			if (batch.size == 0) {
				continue;
			}
			PreparedRenderType type = batch.prepared;
			RenderTarget target = type.outputTarget().getRenderTarget();
			GpuTextureView color = RenderSystem.outputColorTextureOverride != null
					? RenderSystem.outputColorTextureOverride : target.getColorTextureView();
			GpuTextureView depth = target.useDepth
					? (RenderSystem.outputDepthTextureOverride != null ? RenderSystem.outputDepthTextureOverride : target.getDepthTextureView())
					: null;
			int indexCount = batch.vertexCount() / 4 * 6;
			try (RenderPass pass = RenderSystem.getDevice().createCommandEncoder()
					.createRenderPass(() -> "Polonium name tags", color, Optional.empty(), depth, OptionalDouble.empty())) {
				pass.setPipeline(batch.pipeline);
				if (type.scissorState().enabled()) {
					pass.enableScissor(type.scissorState().x(), type.scissorState().y(), type.scissorState().width(), type.scissorState().height());
				}
				RenderSystem.bindDefaultUniforms(pass);
				pass.setUniform("DynamicTransforms", type.dynamicTransforms());
				pass.setUniform("PoloniumInstances", tagData);
				pass.setVertexBuffer(0, vertices.slice(batch.offset, batch.size));
				for (PreparedRenderType.Texture texture : type.textures()) {
					pass.bindTexture(texture.name(), texture.textureView(), texture.sampler());
				}
				pass.setIndexBuffer(indices.getBuffer(indexCount), indices.type());
				pass.drawIndexed(indexCount, 1, 0, 0, 0);
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
			org.slf4j.LoggerFactory.getLogger("Polonium").info("Polonium: {} name tags in {} GPU draws this frame",
					tags.texels() / TEXELS_PER_TAG, draws);
		}
		groups.clear();
		prepared.clear();
		tags.clear();
		current = null;
		uploaded = false;
		frame++;
	}

	/** Packs a run's vertices for the stream (the tag number is stamped per copy). */
	public static byte[] pack(float[] vertices, int[] colors) {
		ByteBuffer out = ByteBuffer.allocate(colors.length * STRIDE).order(ByteOrder.nativeOrder());
		for (int i = 0, v = 0; i < colors.length; i++, v += 5) {
			int c = colors[i];
			out.putFloat(vertices[v]).putFloat(vertices[v + 1]).putFloat(vertices[v + 2]);
			out.put((byte) (c >> 16)).put((byte) (c >> 8)).put((byte) c).put((byte) (c >>> 24));
			out.putFloat(vertices[v + 3]).putFloat(vertices[v + 4]);
			out.putInt(0);
		}
		return out.array();
	}

	/** A batch's append: the run's bytes, with this tag's number in every vertex. */
	private static void stamp(byte[] bytes, int from, int length, int tag) {
		boolean little = ByteOrder.nativeOrder() == ByteOrder.LITTLE_ENDIAN;
		for (int at = from + INSTANCE_OFFSET; at < from + length; at += STRIDE) {
			if (little) {
				bytes[at] = (byte) tag;
				bytes[at + 1] = (byte) (tag >> 8);
				bytes[at + 2] = (byte) (tag >> 16);
				bytes[at + 3] = (byte) (tag >> 24);
			} else {
				bytes[at] = (byte) (tag >> 24);
				bytes[at + 1] = (byte) (tag >> 16);
				bytes[at + 2] = (byte) (tag >> 8);
				bytes[at + 3] = (byte) tag;
			}
		}
	}

	private static void append(Batch batch, byte[] run, int tag) {
		if (batch.size + run.length > batch.bytes.length) {
			batch.bytes = Arrays.copyOf(batch.bytes, Math.max(batch.bytes.length * 2, batch.size + run.length));
		}
		System.arraycopy(run, 0, batch.bytes, batch.size, run.length);
		stamp(batch.bytes, batch.size, run.length, tag);
		batch.size += run.length;
	}
}
