//#if MC >= 26.2
package com.arcticlauncher.polonium;

import com.arcticlauncher.polonium.mixin.DrawAccess;
import com.arcticlauncher.polonium.mixin.StagedVertexBufferAccess;
import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.buffers.GpuBufferSlice;
import com.mojang.blaze3d.systems.CommandEncoder;
import com.mojang.blaze3d.systems.GpuDevice;
import com.mojang.blaze3d.vertex.ByteBufferBuilder;
import com.mojang.blaze3d.vertex.MeshData;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.renderer.StagedVertexBuffer;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * The game's {@code StagedVertexBuffer.uploadDrawsToBuffers}, with its copies
 * and sorts spread over {@link Workers}. Every byte lands where the game's
 * code puts it, so frames are identical; only the work is shared out.
 */
public final class ParallelUpload {
	private static final Logger LOG = LoggerFactory.getLogger("Polonium");
	/**
	 * Below this much entity geometry a frame is cheap on one thread, and
	 * handing out work would cost more than it saves (normal play is far below).
	 */
	private static final int MIN_BYTES = 4 << 20;
	private static final MethodHandle STAGING_POOL = stagingPoolGetter();
	private static volatile boolean disabled;

	private ParallelUpload() {}

	/** Upload in parallel; false to let the game do it (small frames, or Polonium stepped aside). */
	public static boolean upload(StagedVertexBuffer self, GpuDevice device, List<StagedVertexBuffer.Draw> draws,
			GpuBuffer vertexGpuBuffer, @Nullable GpuBuffer indexGpuBuffer, int vertexBufferSize, int indexBufferSize) {
		if (disabled || vertexBufferSize < MIN_BYTES || STAGING_POOL == null || !Compatibility.uploadIsOurs()) {
			return false;
		}
		StagingPool pool = stagingPool(self);
		if (pool == null) {
			return false;
		}
		CommandEncoder encoder = device.createCommandEncoder();
		int stagingBufferSize = vertexBufferSize + indexBufferSize;
		GpuBuffer staging = pool.polonium$acquire(device, stagingBufferSize);
		try (GpuBufferSlice.MappedView view = staging.slice(0L, stagingBufferSize).map(false, true)) {
			ByteBuffer buffer = view.data();
			List<Runnable> jobs;
			try {
				jobs = tasks(buffer, draws, indexGpuBuffer != null, vertexBufferSize);
			} catch (RuntimeException e) {
				// Nothing written yet: the game uploads this frame (and every later one) itself.
				disabled = true;
				LOG.error("Polonium: unexpected vertex layout; using the game's own upload from now on", e);
				return false;
			}
			run(jobs);
			for (StagedVertexBuffer.Draw draw : draws) {
				if (!draw.isEmpty()) {
					((DrawAccess) draw).polonium$freeVertexData();
				}
			}
		}
		encoder.copyToBuffer(staging.slice(0L, vertexBufferSize), vertexGpuBuffer.slice(0L, vertexBufferSize));
		if (indexGpuBuffer != null) {
			encoder.copyToBuffer(staging.slice(vertexBufferSize, indexBufferSize), indexGpuBuffer.slice(0L, indexBufferSize));
		}
		return true;
	}

	/**
	 * The game's two loops as independent jobs: a sort per see-through draw
	 * (slowest, so first), then the vertex copies cut into equal byte ranges,
	 * one per thread.
	 */
	private static List<Runnable> tasks(ByteBuffer buffer, List<StagedVertexBuffer.Draw> draws, boolean hasIndices,
			int vertexBufferSize) {
		List<Runnable> jobs = new ArrayList<>();
		Copies copies = Copies.of(buffer, draws);
		for (StagedVertexBuffer.Draw draw : draws) {
			DrawAccess access = (DrawAccess) draw;
			if (!draw.isEmpty() && hasIndices && access.polonium$quadSorting() != null) {
				int target = vertexBufferSize + access.polonium$indexOffset();
				jobs.add(() -> {
					MeshData.SortState state = new MeshData.SortState(
							StagedVertexBufferAccess.polonium$decodeSortingPoints(draw), access.polonium$indexType());
					state.writeSortedIndexBuffer(view(buffer, target), access.polonium$quadSorting());
				});
			}
		}
		int parts = Workers.HELPERS + 1;
		for (int part = 0; part < parts; part++) {
			int index = part;
			jobs.add(() -> copies.copyPart(index, parts));
		}
		return jobs;
	}

	/** The sorted index buffer's place in the mapped buffer (its own position, the same byte order). */
	private static ByteBuffer view(ByteBuffer buffer, int at) {
		ByteBuffer view = buffer.duplicate().order(buffer.order());
		view.position(at);
		return view;
	}

	/** All jobs on the worker threads; if that ever fails, Polonium steps aside and they run here. */
	private static void run(List<Runnable> tasks) {
		try {
			Workers.runAll(tasks);
		} catch (RuntimeException | Error e) {
			disabled = true;
			LOG.error("Polonium: parallel upload failed; using the game's own upload from now on", e);
			for (Runnable task : tasks) {
				task.run();
			}
		}
	}

	private static @Nullable StagingPool stagingPool(StagedVertexBuffer self) {
		try {
			return (StagingPool) STAGING_POOL.invoke(self);
		} catch (Throwable e) {
			disabled = true;
			LOG.error("Polonium: can't reach the staging buffers; parallel upload is off", e);
			return null;
		}
	}

	private static @Nullable MethodHandle stagingPoolGetter() {
		try {
			Field field = StagedVertexBuffer.class.getDeclaredField("stagingGpuBufferPool");
			field.setAccessible(true);
			return MethodHandles.lookup().unreflectGetter(field);
		} catch (ReflectiveOperationException | RuntimeException e) {
			LOG.error("Polonium: this Minecraft's vertex buffer is different; parallel upload is off", e);
			return null;
		}
	}
}
//#endif
