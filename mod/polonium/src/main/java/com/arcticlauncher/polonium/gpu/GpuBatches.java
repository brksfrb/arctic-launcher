package com.arcticlauncher.polonium.gpu;

import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.buffers.GpuBufferSlice;
import com.mojang.blaze3d.systems.ScissorState;
import net.minecraft.client.renderer.rendertype.OutputTarget;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.systems.CommandEncoder;
import com.mojang.blaze3d.systems.GpuDevice;
import com.mojang.blaze3d.systems.RenderPass;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.textures.GpuTextureView;
import com.mojang.blaze3d.PrimitiveTopology;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.OptionalDouble;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.renderer.rendertype.PreparedRenderType;
import net.minecraft.client.renderer.rendertype.RenderType;
import org.jspecify.annotations.Nullable;
import org.lwjgl.system.MemoryUtil;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Entities drawn on the GPU, for one of the game's feature renderers. While
 * the game prepares a frame, a front end ({@link GpuEntities}, {@link GpuItems})
 * adds each entity it takes over to a batch: entities sharing a mesh and a
 * render type, whose per-entity data (poses, light, colors) is all that's
 * kept. When the game draws a group, its batches are drawn first, each with
 * one instanced call; the GPU places every vertex itself.
 *
 * Render thread only.
 */
public final class GpuBatches implements GpuFeature {
	private static final Logger LOG = LoggerFactory.getLogger("Polonium");
	/** Shaders mods replace the entity pipelines with their own; leave them to it. */
	private static final boolean SHADER_MOD = loaded("iris", "oculus");
	/** Mods that change entity models themselves (animated or custom models); leave them to it. */
	private static final boolean MODEL_MOD = loaded("entity_model_features", "figura");
	/** Meshes unused for this many frames are freed (models from a resource reload, say). */
	static final long MESH_IDLE_FRAMES = 600;
	private static final int FRAMES_IN_FLIGHT = 3;
	private static volatile boolean disabled = SHADER_MOD || MODEL_MOD;
	private static boolean announced;

	private final Runnable evictIdleMeshes;
	/** Runs before the frame's instance data is uploaded (finishing work deferred from {@link #add}). */
	private Runnable beforeUpload = () -> {};
	/** Runs once the frame is drawn. */
	private Runnable afterFrame = () -> {};
	/** What the batches hold, for the log ("entity models", "items"). */
	private String label = "entities";
	private final Map<RenderType, PreparedRenderType> prepared = new IdentityHashMap<>();
	/** Batches per prepared group, in group order (the game's group index). */
	private final List<List<Batch>> groups = new ArrayList<>();
	private final List<Batch> spare = new ArrayList<>();
	private @Nullable List<Batch> current;
	private boolean strictlyOrdered;
	private boolean uploaded;
	private long frame;
	private long lastReport;
	private static final long REPORT_NANOS = 10_000_000_000L;
	private final GpuBuffer[] instanceBuffers = new GpuBuffer[FRAMES_IN_FLIGHT];
	private final GpuBuffer[] drawBuffers = new GpuBuffer[FRAMES_IN_FLIGHT];

	private static final class Batch {
		PreparedRenderType prepared;
		RenderPipeline pipeline;
		GpuMesh mesh;
		/** Entities may join from anywhere in the group (opaque, or the group's order is free). */
		boolean anyOrder;
		final InstanceData data = new InstanceData();
		int instances;
		int firstTexel;
		long drawOffset;
	}

	/** {@code evictIdleMeshes} runs every {@link #MESH_IDLE_FRAMES} frames, to free meshes no longer drawn. */
	GpuBatches(String label, Runnable evictIdleMeshes) {
		this.label = label;
		this.evictIdleMeshes = evictIdleMeshes;
	}

	void beforeUpload(Runnable task) {
		this.beforeUpload = task;
	}

	void afterFrame(Runnable task) {
		this.afterFrame = task;
	}

	static boolean disabled() {
		return disabled;
	}

	static boolean shaderMod() {
		return SHADER_MOD;
	}

	static boolean modelMod() {
		return MODEL_MOD;
	}

	long frame() {
		return frame;
	}

	/** True when a group is being prepared and the GPU path is on. */
	boolean preparing() {
		return current != null && !disabled;
	}

	/** The game is about to prepare a group of model submits. */
	@Override
	public void beginGroup(boolean strictlyOrdered) {
		this.strictlyOrdered = strictlyOrdered;
		List<Batch> group = new ArrayList<>();
		groups.add(group);
		current = group;
	}

	@Override
	public void endGroup() {
		current = null;
	}

	/**
	 * The instance data to add one entity to: the batch for this render type
	 * and mesh, made if needed. The caller writes exactly
	 * {@code mesh.texelsPerInstance} texels to it.
	 */
	InstanceData add(RenderType renderType, GpuMesh mesh) {
		List<Batch> group = java.util.Objects.requireNonNull(current, "not preparing a group");
		mesh.lastUsedFrame = frame;
		PreparedRenderType preparedType = prepared.computeIfAbsent(renderType, RenderType::prepare);
		// As the game: consecutive entities of one render type share a draw; when the
		// group's order is free they share it from anywhere. Opaque ones look the
		// same in any order (the depth test decides), so those share freely too.
		boolean consolidate = renderType.canConsolidateConsecutiveGeometry();
		boolean anyOrder = !strictlyOrdered || !renderType.hasBlending();
		Batch batch = consolidate ? find(group, preparedType, mesh, anyOrder) : null;
		if (batch == null) {
			batch = spare.isEmpty() ? new Batch() : spare.removeLast();
			batch.prepared = preparedType;
			batch.pipeline = InstancedPipelines.twin(preparedType.pipeline(), mesh.vertexShader);
			batch.mesh = mesh;
			batch.anyOrder = anyOrder;
			batch.instances = 0;
			batch.data.clear();
			group.add(batch);
		}
		batch.instances++;
		return batch.data;
	}

	/** The batch to add to: the last one if it matches, or (when order doesn't matter) any matching one. */
	private static @Nullable Batch find(List<Batch> group, PreparedRenderType preparedType, GpuMesh mesh, boolean anyOrder) {
		for (int i = group.size() - 1; i >= 0; i--) {
			Batch batch = group.get(i);
			if (batch.mesh == mesh && batch.prepared.equals(preparedType)) {
				return batch;
			}
			if (!anyOrder || !batch.anyOrder) {
				return null;
			}
		}
		return null;
	}

	/** Draw this group's batches (the game draws the rest of the group after). */
	@Override
	public void executeGroup(int groupIndex) {
		if (groupIndex < 0 || groupIndex >= groups.size() || groups.get(groupIndex).isEmpty()) {
			return;
		}
		try {
			if (!uploaded) {
				beforeUpload.run();
				upload();
				uploaded = true;
			}
			drawAll(groups.get(groupIndex), instanceBuffers[slot()], drawBuffers[slot()]);
		} catch (RuntimeException e) {
			disable("drawing models on the GPU failed", e);
		}
	}

	/** Every batch's instance data in one texel buffer, and each batch's place in it in one uniform buffer. */
	private void upload() {
		GpuDevice device = RenderSystem.getDevice();
		int alignment = Math.max(16, device.getDeviceInfo().limits().minUniformOffsetAlignment());
		long texels = 0;
		long drawBytes = 0;
		for (List<Batch> group : groups) {
			for (Batch batch : group) {
				batch.firstTexel = (int) texels;
				texels += batch.data.texels();
				batch.drawOffset = drawBytes;
				drawBytes += alignment;
			}
		}
		if (texels == 0) {
			return;
		}
		int slot = slot();
		instanceBuffers[slot] = ensure(device, instanceBuffers[slot], texels * 16,
				GpuBuffer.USAGE_UNIFORM_TEXEL_BUFFER | GpuBuffer.USAGE_MAP_WRITE, "Polonium entity instances");
		drawBuffers[slot] = ensure(device, drawBuffers[slot], drawBytes,
				GpuBuffer.USAGE_UNIFORM | GpuBuffer.USAGE_COPY_DST, "Polonium entity draws");
		ByteBuffer drawBytesBuffer = MemoryUtil.memCalloc((int) drawBytes).order(ByteOrder.nativeOrder());
		try (GpuBufferSlice.MappedView view = instanceBuffers[slot].slice(0, texels * 16).map(false, true)) {
			ByteBuffer instanceBytes = view.data().order(ByteOrder.nativeOrder());
			for (List<Batch> group : groups) {
				for (Batch batch : group) {
					batch.data.writeTo(instanceBytes);
					drawBytesBuffer.putInt((int) batch.drawOffset, batch.firstTexel);
					drawBytesBuffer.putInt((int) batch.drawOffset + 4, batch.mesh.texelsPerInstance);
				}
			}
			device.createCommandEncoder().writeToBuffer(drawBuffers[slot].slice(0, drawBytes), drawBytesBuffer);
		} finally {
			MemoryUtil.memFree(drawBytesBuffer);
		}
	}

	private static GpuBuffer ensure(GpuDevice device, @Nullable GpuBuffer buffer, long size, int usage, String label) {
		if (buffer != null && !buffer.isClosed() && buffer.size() >= size) {
			return buffer;
		}
		if (buffer != null) {
			buffer.close();
		}
		// Room to grow, so a crowd that changes a little doesn't reallocate every frame.
		long capacity = Math.max(size + size / 2, 64 * 1024);
		return device.createBuffer(() -> label, usage, capacity);
	}

	/**
	 * A group's batches, each one instanced draw set up the way the game draws
	 * its render type, sharing a render pass while they draw to the same target.
	 */
	private static void drawAll(List<Batch> batches, GpuBuffer instances, GpuBuffer draws) {
		RenderSystem.AutoStorageIndexBuffer indices = RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS);
		RenderPass pass = null;
		OutputTarget passTarget = null;
		ScissorState passScissor = null;
		try {
			for (Batch batch : batches) {
				PreparedRenderType type = batch.prepared;
				if (pass == null || type.outputTarget() != passTarget || !type.scissorState().equals(passScissor)) {
					if (pass != null) {
						pass.close();
					}
					pass = openPass(type, instances);
					passTarget = type.outputTarget();
					passScissor = type.scissorState();
				}
				int indexCount = batch.mesh.vertexCount / 4 * 6;
				pass.setPipeline(batch.pipeline);
				pass.setUniform("DynamicTransforms", type.dynamicTransforms());
				pass.setUniform("PoloniumDraw", draws.slice(batch.drawOffset, 16));
				pass.setVertexBuffer(0, batch.mesh.vertices.slice());
				for (PreparedRenderType.Texture texture : type.textures()) {
					pass.bindTexture(texture.name(), texture.textureView(), texture.sampler());
				}
				pass.setIndexBuffer(indices.getBuffer(indexCount), indices.type());
				pass.drawIndexed(indexCount, batch.instances, 0, 0, 0);
			}
		} finally {
			if (pass != null) {
				pass.close();
			}
		}
	}

	private static RenderPass openPass(PreparedRenderType type, GpuBuffer instances) {
		RenderTarget target = type.outputTarget().getRenderTarget();
		GpuTextureView color = RenderSystem.outputColorTextureOverride != null
				? RenderSystem.outputColorTextureOverride : target.getColorTextureView();
		GpuTextureView depth = target.useDepth
				? (RenderSystem.outputDepthTextureOverride != null ? RenderSystem.outputDepthTextureOverride : target.getDepthTextureView())
				: null;
		RenderPass pass = RenderSystem.getDevice().createCommandEncoder()
				.createRenderPass(() -> "Polonium instanced entities", color, Optional.empty(), depth, OptionalDouble.empty());
		if (type.scissorState().enabled()) {
			pass.enableScissor(type.scissorState().x(), type.scissorState().y(), type.scissorState().width(), type.scissorState().height());
		}
		RenderSystem.bindDefaultUniforms(pass);
		pass.setUniform("PoloniumInstances", instances);
		return pass;
	}

	/** The frame's draws are done: get ready for the next. */
	@Override
	public void endFrame() {
		int entities = 0;
		int draws = 0;
		for (List<Batch> group : groups) {
			for (Batch batch : group) {
				entities += batch.instances;
			}
			draws += group.size();
			spare.addAll(group);
		}
		report(entities, draws);
		afterFrame.run();
		groups.clear();
		prepared.clear();
		current = null;
		uploaded = false;
		frame++;
		if (frame % MESH_IDLE_FRAMES == 0) {
			evictIdleMeshes.run();
		}
	}

	/** Every ten seconds or so, how many entities took how many draws (for checking batching works). */
	private void report(int entities, int draws) {
		String what = label;
		long now = System.nanoTime();
		if (entities > 0 && now - lastReport > REPORT_NANOS) {
			lastReport = now;
			LOG.info("Polonium: {} {} in {} GPU draws this frame", entities, what, draws);
		}
	}

	private int slot() {
		return (int) (frame % FRAMES_IN_FLIGHT);
	}

	/** Free the meshes in {@code meshes} not drawn for a while. */
	static void evictIdle(Map<?, ? extends GpuMesh> meshes, long frame) {
		Iterator<? extends GpuMesh> it = meshes.values().iterator();
		while (it.hasNext()) {
			GpuMesh mesh = it.next();
			if (frame - mesh.lastUsedFrame > MESH_IDLE_FRAMES) {
				mesh.close();
				it.remove();
			}
		}
	}

	static void disable(String what, Throwable e) {
		if (!disabled) {
			disabled = true;
			LOG.error("Polonium: {}; models are drawn the game's way from now on", what, e);
		}
	}

	/** Logged once, so it's clear in the log which path entities take. */
	public static void announce() {
		if (!announced) {
			announced = true;
			LOG.info(SHADER_MOD ? "Polonium: a shaders mod is installed; entity models stay on the game's renderer"
					: MODEL_MOD ? "Polonium: a mod that changes entity models is installed; they stay on the game's renderer"
					: "Polonium: entity models drawn on the GPU");
		}
	}

	private static boolean loaded(String... ids) {
		for (String id : ids) {
			if (FabricLoader.getInstance().isModLoaded(id)) {
				return true;
			}
		}
		return false;
	}
}
