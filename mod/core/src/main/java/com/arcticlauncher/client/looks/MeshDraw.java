package com.arcticlauncher.client.looks;

import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * Sculpted (mesh) cosmetics: triangles drawn through the same entity render
 * types as everything else. Each root node follows the player part it is
 * named after (as for bones), the idle animation runs on the node
 * transforms, and every surface is lit on whichever side the camera sees
 * (a mesh's triangle winding doesn't matter). Version-neutral: the game
 * version supplies the {@link Xform}.
 */
public final class MeshDraw {
	/** Bedrock → Java: y flips around the model's height. */
	private static final float MODEL_HEIGHT = 24f;
	private static final float UNIT = 1f / 16f;

	/** The primitives that follow one player part. */
	public static final class Group {
		public final Attach attach;
		public final int[] primitives;

		Group(Attach attach, int[] primitives) {
			this.attach = attach;
			this.primitives = primitives;
		}
	}

	private MeshDraw() {}

	/** Group the mesh's primitives by the player part their root node follows. */
	public static Group[] groups(MeshModel mesh) {
		Map<Attach, List<Integer>> byPart = new EnumMap<Attach, List<Integer>>(Attach.class);
		for (int i = 0; i < mesh.primitives.size(); i++) {
			int node = mesh.primitives.get(i).node;
			while (mesh.nodes.get(node).parent >= 0) {
				node = mesh.nodes.get(node).parent;
			}
			Attach attach = Attach.of(mesh.nodes.get(node).name);
			List<Integer> list = byPart.get(attach);
			if (list == null) {
				list = new ArrayList<Integer>();
				byPart.put(attach, list);
			}
			list.add(i);
		}
		Group[] groups = new Group[byPart.size()];
		int g = 0;
		for (Map.Entry<Attach, List<Integer>> e : byPart.entrySet()) {
			int[] list = new int[e.getValue().size()];
			for (int k = 0; k < list.length; k++) {
				list[k] = e.getValue().get(k);
			}
			groups[g++] = new Group(e.getKey(), list);
		}
		return groups;
	}

	/** The node matrices to draw with at the moment ({@code animate} off: the authored rest pose). */
	public static float[][] pose(MeshModel mesh, boolean animate) {
		return mesh.world(animate ? (System.currentTimeMillis() % 3_600_000L) / 1000f : -1f);
	}

	/**
	 * Draw one primitive. {@code glow}: the emissive pass (full bright, no
	 * tint); otherwise lit by {@code light} and tinted by the vertex and
	 * material colors.
	 */
	public static void emit(MeshModel mesh, int primitive, Attach attach, boolean animate, Xform xf, int light, boolean glow) {
		draw(mesh, primitive, attach, animate, xf, light, glow ? GLOW : BASE);
	}

	/**
	 * The mesh's travelling light (see {@link MeshModel.Sheen}) for one primitive: the same triangles,
	 * full bright, tinted, with the band's brightness as each vertex's alpha. Nothing to draw (and
	 * nothing drawn) when the mesh has no sheen.
	 */
	public static void emitSheen(MeshModel mesh, int primitive, Attach attach, Xform xf) {
		if (mesh.sheen != null) {
			draw(mesh, primitive, attach, true, xf, Xform.FULL_BRIGHT, SHEEN);
		}
	}

	private static final int BASE = 0;
	private static final int GLOW = 1;
	private static final int SHEEN = 2;

	private static void draw(MeshModel mesh, int primitive, Attach attach, boolean animate, Xform xf, int light, int mode) {
		boolean glow = mode != BASE;
		// Worked out when the triangles are drawn, so the idle animation runs on the frame being drawn.
		float[][] world = pose(mesh, animate);
		MeshModel.Primitive p = mesh.primitives.get(primitive);
		float[] m = world[p.node];
		MeshModel.Material material = p.material < 0 ? null : mesh.materials.get(p.material);
		float[] base = material == null ? new float[] {1, 1, 1, 1} : material.baseColor;
		int n = p.positions.length / 3;
		// The vertices in Java model space, relative to the player part's pivot, in pixels.
		float[] pos = new float[n * 3];
		float[] nrm = new float[n * 3];
		float[] bright = mode == SHEEN ? new float[n] : null;
		float clock = (System.currentTimeMillis() % 3_600_000L) / 1000f;
		for (int i = 0; i < n; i++) {
			float x = p.positions[i * 3];
			float y = p.positions[i * 3 + 1];
			float z = p.positions[i * 3 + 2];
			float wx = m[0] * x + m[1] * y + m[2] * z + m[3];
			float wy = m[4] * x + m[5] * y + m[6] * z + m[7];
			float wz = m[8] * x + m[9] * y + m[10] * z + m[11];
			pos[i * 3] = wx - attach.x;
			pos[i * 3 + 1] = (MODEL_HEIGHT - wy) - attach.y;
			pos[i * 3 + 2] = wz - attach.z;
			float nx = p.normals[i * 3];
			float ny = p.normals[i * 3 + 1];
			float nz = p.normals[i * 3 + 2];
			float rx = m[0] * nx + m[1] * ny + m[2] * nz;
			float ry = m[4] * nx + m[5] * ny + m[6] * nz;
			float rz = m[8] * nx + m[9] * ny + m[10] * nz;
			float len = (float) Math.sqrt(rx * rx + ry * ry + rz * rz);
			len = len < 1e-9f ? 1f : len;
			nrm[i * 3] = rx / len;
			nrm[i * 3 + 1] = -ry / len;
			nrm[i * 3 + 2] = rz / len;
			if (bright != null) {
				bright[i] = mesh.sheen.at(wx, wy, wz, nrm[i * 3 + 2], clock);
			}
		}
		int tint = 0;
		if (bright != null) {
			float[] c = mesh.sheen.tint;
			tint = channel(c[0]) << 16 | channel(c[1]) << 8 | channel(c[2]);
		}
		float[] a = new float[3];
		float[] b = new float[3];
		float[] c = new float[3];
		int[] idx = p.indices;
		for (int t = 0; t + 2 < idx.length; t += 3) {
			int i0 = idx[t];
			int i1 = idx[t + 1];
			int i2 = idx[t + 2];
			if (bright != null && bright[i0] < 0.004f && bright[i1] < 0.004f && bright[i2] < 0.004f) {
				continue;
			}
			at(xf, pos, i0, a);
			at(xf, pos, i1, b);
			at(xf, pos, i2, c);
			// Which way the winding faces (in view space), aligned with the mesh's own normals.
			float ux = b[0] - a[0];
			float uy = b[1] - a[1];
			float uz = b[2] - a[2];
			float vx = c[0] - a[0];
			float vy = c[1] - a[1];
			float vz = c[2] - a[2];
			float gx = uy * vz - uz * vy;
			float gy = uz * vx - ux * vz;
			float gz = ux * vy - uy * vx;
			// The mesh's normal for this triangle, in view space.
			xf.normal(nrm[i0 * 3] + nrm[i1 * 3] + nrm[i2 * 3], nrm[i0 * 3 + 1] + nrm[i1 * 3 + 1] + nrm[i2 * 3 + 1],
					nrm[i0 * 3 + 2] + nrm[i1 * 3 + 2] + nrm[i2 * 3 + 2]);
			float outward = gx * xf.x + gy * xf.y + gz * xf.z >= 0 ? 1f : -1f;
			// Facing the camera (which sits at the view-space origin)?
			float toCamera = -(gx * a[0] + gy * a[1] + gz * a[2]) * outward;
			float flip = toCamera >= 0 ? 1f : -1f;
			for (int k = 0; k < 4; k++) {
				int i = k == 0 ? i0 : k == 1 ? i1 : i2;
				float u = p.uvs[i * 2];
				float v = p.uvs[i * 2 + 1];
				int color = bright != null ? channel(bright[i]) << 24 | tint : glow ? -1 : pack(p.colors, i, base);
				xf.normal(nrm[i * 3] * flip, nrm[i * 3 + 1] * flip, nrm[i * 3 + 2] * flip);
				float[] at = k == 0 ? a : k == 1 ? b : c;
				xf.vertex(at[0], at[1], at[2], color, u, v, glow ? Xform.FULL_BRIGHT : light, xf.x, xf.y, xf.z);
			}
		}
	}

	/** Vertex {@code i} (model pixels) turned by the pose, into {@code out}. */
	private static void at(Xform xf, float[] pos, int i, float[] out) {
		xf.position(pos[i * 3] * UNIT, pos[i * 3 + 1] * UNIT, pos[i * 3 + 2] * UNIT);
		out[0] = xf.x;
		out[1] = xf.y;
		out[2] = xf.z;
	}

	private static int pack(float[] colors, int i, float[] base) {
		int r = channel(colors[i * 4] * base[0]);
		int g = channel(colors[i * 4 + 1] * base[1]);
		int b = channel(colors[i * 4 + 2] * base[2]);
		int a = channel(colors[i * 4 + 3] * base[3]);
		return a << 24 | r << 16 | g << 8 | b;
	}

	private static int channel(float v) {
		return Math.max(0, Math.min(255, Math.round(v * 255f)));
	}
}
