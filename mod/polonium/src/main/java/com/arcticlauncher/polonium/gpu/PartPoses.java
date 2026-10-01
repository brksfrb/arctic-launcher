package com.arcticlauncher.polonium.gpu;

import com.arcticlauncher.polonium.Workers;
import com.mojang.blaze3d.vertex.PoseStack;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
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
 * object serves every entity, so each entity's part values are copied out
 * then ({@link #snapshot}, render thread). The matrix math for all of them
 * runs later on several threads ({@link #computeAll}).
 */
final class PartPoses {
	/** Texels per part: the pose's three rows (normals are worked out from it on the GPU). */
	static final int TEXELS_PER_PART = 3;
	/** Per part: x, y, z, xRot, yRot, zRot, xScale, yScale, zScale, visible, skipDraw. */
	private static final int VALUES_PER_PART = 11;
	/** Before the parts: the root pose (16), then color, overlay and light (as int bits). */
	private static final int HEADER = 19;
	/** Below this many entities the threads aren't worth waking. */
	private static final int PARALLEL_MIN = 64;

	private float[] values = new float[16 * 1024];
	private int used;
	private final List<Job> jobs = new ArrayList<>();
	private int jobCount;
	private final ThreadLocal<List<Matrix4f>> stacks = ThreadLocal.withInitial(ArrayList::new);

	private static final class Job {
		ModelMesh mesh;
		InstanceData target;
		int targetOffset;
		int valuesOffset;
	}

	/** Copy one entity's pose out now (render thread); its instance data is written by {@link #computeAll}. */
	void snapshot(ModelMesh mesh, PoseStack.Pose root, int color, int overlayCoords, int lightCoords, InstanceData target) {
		int start = used;
		grow(HEADER + mesh.parts.length * VALUES_PER_PART);
		root.pose().get(values, start);
		values[start + 16] = Float.intBitsToFloat(color);
		values[start + 17] = Float.intBitsToFloat(overlayCoords);
		values[start + 18] = Float.intBitsToFloat(lightCoords);
		int at = start + HEADER;
		for (ModelPart part : mesh.parts) {
			values[at] = part.x;
			values[at + 1] = part.y;
			values[at + 2] = part.z;
			values[at + 3] = part.xRot;
			values[at + 4] = part.yRot;
			values[at + 5] = part.zRot;
			values[at + 6] = part.xScale;
			values[at + 7] = part.yScale;
			values[at + 8] = part.zScale;
			values[at + 9] = part.visible ? 1 : 0;
			values[at + 10] = part.skipDraw ? 1 : 0;
			at += VALUES_PER_PART;
		}
		used = at;
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
		}
		jobCount = 0;
	}

	private void compute(int from, int to) {
		List<Matrix4f> stack = stacks.get();
		Matrix4f root = at(stack, 0);
		for (int i = from; i < to; i++) {
			Job job = jobs.get(i);
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
			root.set(values, v);
			part(job.mesh, 0, root, 1, v + HEADER, out, o + 8, stack);
		}
	}

	/** Write part {@code index} (under {@code parent}) and its subtree; returns the number after the subtree. */
	private int part(ModelMesh mesh, int index, Matrix4f parent, int depth, int partValues, float[] out, int outStart,
			List<Matrix4f> stack) {
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
			next = part(mesh, next, m, depth + 1, partValues, out, outStart, stack);
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
