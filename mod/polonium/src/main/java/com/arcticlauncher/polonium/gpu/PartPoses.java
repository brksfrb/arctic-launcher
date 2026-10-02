package com.arcticlauncher.polonium.gpu;

import com.arcticlauncher.polonium.Workers;
import com.mojang.blaze3d.vertex.PoseStack;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.ModelPart;
import org.joml.Matrix4f;

/**
 * Entities' instance data: color, overlay and light, then every part's pose,
 * worked out as {@link ModelPart#render} would (translate, rotate Z-Y-X,
 * scale; the same visibility rules) but written as matrices instead of
 * vertices. Parts that wouldn't be drawn get zero matrices, which collapse
 * their faces to nothing.
 *
 * The game poses a model (setupAnim) right before drawing it, and one model
 * object serves every entity. Models that can be copied ({@link ModelCopies})
 * are posed later on the helper threads, each with its own copy, using the
 * game's own setupAnim ({@link #defer}); others are posed on the render thread
 * and their part values copied out then ({@link #snapshot}). The matrix math
 * for all of them runs on several threads ({@link #computeAll}).
 */
final class PartPoses {
	/** Texels per part: the pose's three rows (normals are worked out from it on the GPU). */
	static final int TEXELS_PER_PART = 3;
	/** Per part: x, y, z, xRot, yRot, zRot, xScale, yScale, zScale, visible, skipDraw. */
	private static final int VALUES_PER_PART = 11;
	/**
	 * Before the parts: the root pose (16), color, overlay and light (as int
	 * bits), then where the texture is: u, v offset and scale (an atlas cell;
	 * 0, 0, 1, 1 for the texture itself).
	 */
	private static final int HEADER = 23;
	/** Below this many entities the threads aren't worth waking. */
	private static final int PARALLEL_MIN = 64;

	private float[] values = new float[16 * 1024];
	private int used;
	private final List<Job> jobs = new ArrayList<>();
	private int jobCount;
	private final ThreadLocal<List<Matrix4f>> stacks = ThreadLocal.withInitial(ArrayList::new);

	private final ThreadLocal<float[]> scratch = ThreadLocal.withInitial(() -> new float[VALUES_PER_PART * 64]);

	private static final class Job {
		ModelMesh mesh;
		InstanceData target;
		int targetOffset;
		int valuesOffset;
		/** Posed on a helper thread (with a copy of this model, from this state), or null: already snapshotted. */
		@org.jspecify.annotations.Nullable Model<?> model;
		@org.jspecify.annotations.Nullable Object state;
	}

	/** Copy one entity's pose out now (render thread, model already posed); its instance data is written by {@link #computeAll}. */
	void snapshot(ModelMesh mesh, PoseStack.Pose root, int color, int overlayCoords, int lightCoords, float[] uv, InstanceData target) {
		int start = header(mesh.parts.length, root, color, overlayCoords, lightCoords, uv);
		used = copyParts(mesh.parts, values, start + HEADER);
		job(mesh, target, start, null, null);
	}

	/** Pose this entity later, on a helper thread, with a copy of {@code model} (see {@link ModelCopies}). */
	void defer(ModelMesh mesh, Model<?> model, Object state, PoseStack.Pose root, int color, int overlayCoords, int lightCoords,
			float[] uv, InstanceData target) {
		int start = header(0, root, color, overlayCoords, lightCoords, uv);
		used = start + HEADER;
		job(mesh, target, start, model, state);
	}

	/** The root pose, color, overlay and light; room for {@code parts} parts after it. */
	private int header(int parts, PoseStack.Pose root, int color, int overlayCoords, int lightCoords, float[] uv) {
		int start = used;
		grow(HEADER + parts * VALUES_PER_PART);
		root.pose().get(values, start);
		values[start + 16] = Float.intBitsToFloat(color);
		values[start + 17] = Float.intBitsToFloat(overlayCoords);
		values[start + 18] = Float.intBitsToFloat(lightCoords);
		values[start + 19] = uv[0];
		values[start + 20] = uv[1];
		values[start + 21] = uv[2];
		values[start + 22] = uv[3];
		return start;
	}

	/** The parts' pose values into {@code out} from {@code at}; returns the index after them. */
	private static int copyParts(ModelPart[] parts, float[] out, int at) {
		for (ModelPart part : parts) {
			out[at] = part.x;
			out[at + 1] = part.y;
			out[at + 2] = part.z;
			out[at + 3] = part.xRot;
			out[at + 4] = part.yRot;
			out[at + 5] = part.zRot;
			out[at + 6] = part.xScale;
			out[at + 7] = part.yScale;
			out[at + 8] = part.zScale;
			out[at + 9] = part.visible ? 1 : 0;
			out[at + 10] = part.skipDraw ? 1 : 0;
			at += VALUES_PER_PART;
		}
		return at;
	}

	private void job(ModelMesh mesh, InstanceData target, int start, @org.jspecify.annotations.Nullable Model<?> model,
			@org.jspecify.annotations.Nullable Object state) {
		Job job;
		if (jobCount < jobs.size()) {
			job = jobs.get(jobCount);
		} else {
			job = new Job();
			jobs.add(job);
		}
		jobCount++;
		job.mesh = mesh;
		job.target = target;
		job.targetOffset = target.reserve(mesh.texelsPerInstance);
		job.valuesOffset = start;
		job.model = model;
		job.state = state;
	}

	/** Write every snapshot's instance data, on several threads when there are many. */
	void computeAll() {
		int count = jobCount;
		if (count == 0) {
			return;
		}
		if (count < PARALLEL_MIN) {
			compute(0, count);
		} else {
			int parts = Workers.HELPERS + 1;
			List<Runnable> chunks = new ArrayList<>(parts);
			for (int p = 0; p < parts; p++) {
				int from = count * p / parts;
				int to = count * (p + 1) / parts;
				chunks.add(() -> compute(from, to));
			}
			Workers.runAll(chunks);
		}
	}

	/** Start over for the next frame. */
	void clear() {
		used = 0;
		for (int i = 0; i < jobCount; i++) {
			jobs.get(i).target = null;
			jobs.get(i).mesh = null;
			jobs.get(i).model = null;
			jobs.get(i).state = null;
		}
		jobCount = 0;
	}

	@SuppressWarnings({"unchecked", "rawtypes"})
	private void compute(int from, int to) {
		List<Matrix4f> stack = stacks.get();
		Matrix4f root = at(stack, 0);
		for (int i = from; i < to; i++) {
			Job job = jobs.get(i);
			float[] parts = values;
			int partsAt = job.valuesOffset + HEADER;
			if (job.model != null) {
				// The game's own posing, on this thread's copy of the model.
				ModelCopies.Copy copy = ModelCopies.forThisThread(job.model, job.mesh.parts);
				((Model) copy.model).setupAnim(job.state);
				parts = scratch.get();
				if (parts.length < copy.parts.length * VALUES_PER_PART) {
					parts = new float[copy.parts.length * VALUES_PER_PART];
					scratch.set(parts);
				}
				copyParts(copy.parts, parts, 0);
				partsAt = 0;
			}
			float[] out = job.target.array();
			int o = job.targetOffset * 4;
			int v = job.valuesOffset;
			int color = Float.floatToRawIntBits(values[v + 16]);
			int overlay = Float.floatToRawIntBits(values[v + 17]);
			int light = Float.floatToRawIntBits(values[v + 18]);
			out[o] = ((color >> 16) & 0xFF) / 255f;
			out[o + 1] = ((color >> 8) & 0xFF) / 255f;
			out[o + 2] = (color & 0xFF) / 255f;
			out[o + 3] = ((color >>> 24) & 0xFF) / 255f;
			out[o + 4] = overlay & 0xFFFF;
			out[o + 5] = (overlay >>> 16) & 0xFFFF;
			out[o + 6] = light & 0xFFFF;
			out[o + 7] = (light >>> 16) & 0xFFFF;
			out[o + 8] = values[v + 19];
			out[o + 9] = values[v + 20];
			out[o + 10] = values[v + 21];
			out[o + 11] = values[v + 22];
			root.set(values, v);
			part(job.mesh, parts, 0, root, 1, partsAt, out, o + 12, stack);
		}
	}

	/** Write part {@code index} (under {@code parent}) and its subtree; returns the number after the subtree. */
	private static int part(ModelMesh mesh, float[] values, int index, Matrix4f parent, int depth, int partValues, float[] out,
			int outStart, List<Matrix4f> stack) {
		int end = mesh.subtreeEnd[index];
		int v = partValues + index * VALUES_PER_PART;
		int o = outStart + index * TEXELS_PER_PART * 4;
		boolean visible = values[v + 9] != 0;
		boolean drawn = visible && (!ModelMesh.cubes(mesh.parts[index]).isEmpty() || end > index + 1);
		if (!drawn) {
			Arrays.fill(out, o, outStart + end * TEXELS_PER_PART * 4, 0f);
			return end;
		}
		Matrix4f m = at(stack, depth).set(parent).translate(values[v] / 16.0F, values[v + 1] / 16.0F, values[v + 2] / 16.0F);
		float xRot = values[v + 3];
		float yRot = values[v + 4];
		float zRot = values[v + 5];
		if (xRot != 0.0F || yRot != 0.0F || zRot != 0.0F) {
			m.rotateZYX(zRot, yRot, xRot);
		}
		float xScale = values[v + 6];
		float yScale = values[v + 7];
		float zScale = values[v + 8];
		if (xScale != 1.0F || yScale != 1.0F || zScale != 1.0F) {
			m.scale(xScale, yScale, zScale);
		}
		if (values[v + 10] != 0) {
			Arrays.fill(out, o, o + TEXELS_PER_PART * 4, 0f);
		} else {
			out[o] = m.m00();
			out[o + 1] = m.m10();
			out[o + 2] = m.m20();
			out[o + 3] = m.m30();
			out[o + 4] = m.m01();
			out[o + 5] = m.m11();
			out[o + 6] = m.m21();
			out[o + 7] = m.m31();
			out[o + 8] = m.m02();
			out[o + 9] = m.m12();
			out[o + 10] = m.m22();
			out[o + 11] = m.m32();
		}
		int next = index + 1;
		while (next < end) {
			next = part(mesh, values, next, m, depth + 1, partValues, out, outStart, stack);
		}
		return end;
	}

	private void grow(int more) {
		if (used + more > values.length) {
			values = Arrays.copyOf(values, Math.max(values.length * 2, used + more));
		}
	}

	private static Matrix4f at(List<Matrix4f> stack, int depth) {
		while (stack.size() <= depth) {
			stack.add(new Matrix4f());
		}
		return stack.get(depth);
	}
}
