package com.arcticlauncher.client.looks;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Blockbench (Bedrock) geometry turned into quads we draw ourselves, so a
 * face can have its own texture rectangle and a cube its own turn (vanilla
 * model parts can't). Bedrock files put the feet at y = 0 with y going up;
 * Java models put the head's pivot at y = 0 with y going down, so y becomes
 * 24 - y. Each root bone hangs off the player part of the same name (head,
 * body, rightArm, …), and bones move like Minecraft's model parts: move to
 * the pivot, then turn Z·Y·X. Version-neutral: the game version supplies the
 * {@link Xform}.
 */
public final class CuboidModel {
	/** Bedrock → Java: y flips around the model's height. */
	private static final float MODEL_HEIGHT = 24f;
	private static final float DEG = (float) (Math.PI / 180);
	/** Model units (pixels) to blocks. */
	private static final float UNIT = 1f / 16f;
	/** Floats per quad: 4 × (x, y, z, u, v), then the normal. */
	private static final int QUAD = 4 * 5 + 3;

	private CuboidModel() {}

	/** One root bone's model (with its child bones), drawn attached to a player part. */
	public static final class Piece {
		public final Attach attach;
		private final int[] parent;
		/** Each bone's pivot relative to its parent's pivot (a root's: relative to the player part), in pixels. */
		private final float[][] offset;
		/** Each bone's own rotation in radians. */
		private final float[][] rotation;
		private final String[] names;
		/** Per bone: its quads, positions relative to its own pivot, in pixels. */
		private final float[][] quads;
		private final Animation idle;

		Piece(Attach attach, int[] parent, float[][] offset, float[][] rotation, String[] names, float[][] quads, Animation idle) {
			this.attach = attach;
			this.parent = parent;
			this.offset = offset;
			this.rotation = rotation;
			this.names = names;
			this.quads = quads;
			this.idle = idle;
		}

		/**
		 * Draws the piece with the pose of its player part. {@code light} is
		 * the packed light; glow pieces pass fullbright.
		 */
		public void emit(Xform xf, int light) {
			float t = idle == null ? 0f : (System.currentTimeMillis() % 3_600_000L) / 1000f;
			emit(xf, light, idle, t);
		}

		/** Draws the piece with its bones posed by {@code motion} (bones by name) at {@code t} seconds. */
		public void emit(Xform xf, int light, Animation motion, float t) {
			Animation idle = motion;
			int n = parent.length;
			float[] world = new float[n * 12];
			for (int i = 0; i < n; i++) {
				float rx = rotation[i][0];
				float ry = rotation[i][1];
				float rz = rotation[i][2];
				float ox = offset[i][0];
				float oy = offset[i][1];
				float oz = offset[i][2];
				if (idle != null) {
					float[] r = idle.rotation(names[i], t);
					if (r != null) {
						rx += r[0] * DEG;
						ry += r[1] * DEG;
						rz += r[2] * DEG;
					}
					float[] p = idle.position(names[i], t);
					if (p != null) {
						ox += p[0];
						oy -= p[1];
						oz += p[2];
					}
				}
				place(world, i, parent[i], rx, ry, rz, ox, oy, oz);
			}
			for (int i = 0; i < n; i++) {
				float[] q = quads[i];
				int w = i * 12;
				for (int at = 0; at + QUAD <= q.length; at += QUAD) {
					float sx = q[at + 20];
					float sy = q[at + 21];
					float sz = q[at + 22];
					xf.normal(world[w] * sx + world[w + 1] * sy + world[w + 2] * sz, world[w + 3] * sx + world[w + 4] * sy + world[w + 5] * sz,
							world[w + 6] * sx + world[w + 7] * sy + world[w + 8] * sz);
					float outX = xf.x;
					float outY = xf.y;
					float outZ = xf.z;
					for (int c = 0; c < 4; c++) {
						int v = at + c * 5;
						float x = q[v];
						float y = q[v + 1];
						float z = q[v + 2];
						float px = world[w] * x + world[w + 1] * y + world[w + 2] * z + world[w + 9];
						float py = world[w + 3] * x + world[w + 4] * y + world[w + 5] * z + world[w + 10];
						float pz = world[w + 6] * x + world[w + 7] * y + world[w + 8] * z + world[w + 11];
						xf.position(px * UNIT, py * UNIT, pz * UNIT);
						xf.vertex(xf.x, xf.y, xf.z, -1, q[v + 3], q[v + 4], light, outX, outY, outZ);
					}
				}
			}
		}

		/** Bone {@code i}'s world matrix (3×3 rows, then the translation) from its parent's. */
		private static void place(float[] world, int i, int parent, float rx, float ry, float rz, float ox, float oy, float oz) {
			float sx = (float) Math.sin(rx);
			float cx = (float) Math.cos(rx);
			float sy = (float) Math.sin(ry);
			float cy = (float) Math.cos(ry);
			float sz = (float) Math.sin(rz);
			float cz = (float) Math.cos(rz);
			// Z·Y·X, like Minecraft's rotationZYX.
			float[] l = {cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx,
					sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx,
					-sy, cy * sx, cy * cx};
			int w = i * 12;
			if (parent < 0) {
				System.arraycopy(l, 0, world, w, 9);
				world[w + 9] = ox;
				world[w + 10] = oy;
				world[w + 11] = oz;
				return;
			}
			int p = parent * 12;
			for (int r = 0; r < 3; r++) {
				for (int c = 0; c < 3; c++) {
					world[w + r * 3 + c] = world[p + r * 3] * l[c] + world[p + r * 3 + 1] * l[3 + c] + world[p + r * 3 + 2] * l[6 + c];
				}
				world[w + 9 + r] = world[p + r * 3] * ox + world[p + r * 3 + 1] * oy + world[p + r * 3 + 2] * oz + world[p + 9 + r];
			}
		}
	}

	/** Build the quads of every root bone of the geometry. */
	public static List<Piece> bake(Geometry geometry, Animation idle) {
		List<Piece> pieces = new ArrayList<Piece>();
		for (Geometry.Bone root : geometry.bones) {
			if (root.parent != null) {
				continue;
			}
			List<Geometry.Bone> order = new ArrayList<Geometry.Bone>();
			collect(geometry, root, order);
			Attach attach = Attach.of(root.name);
			int n = order.size();
			int[] parent = new int[n];
			float[][] offset = new float[n][];
			float[][] rotation = new float[n][];
			String[] names = new String[n];
			float[][] quads = new float[n][];
			for (int i = 0; i < n; i++) {
				Geometry.Bone bone = order.get(i);
				float[] pivot = pivot(bone);
				names[i] = bone.name;
				parent[i] = -1;
				float[] from = {attach.x, attach.y, attach.z};
				for (int j = 0; j < i; j++) {
					if (order.get(j).name.equals(bone.parent)) {
						parent[i] = j;
						from = pivot(order.get(j));
					}
				}
				offset[i] = new float[] {pivot[0] - from[0], pivot[1] - from[1], pivot[2] - from[2]};
				rotation[i] = new float[] {bone.rotation[0] * DEG, bone.rotation[1] * DEG, bone.rotation[2] * DEG};
				quads[i] = quads(geometry, bone, pivot);
			}
			pieces.add(new Piece(attach, parent, offset, rotation, names, quads, idle));
		}
		return Collections.unmodifiableList(pieces);
	}

	/** {@code bone} and its descendants, parents first. */
	private static void collect(Geometry geometry, Geometry.Bone bone, List<Geometry.Bone> out) {
		out.add(bone);
		for (Geometry.Bone child : geometry.bones) {
			if (bone.name.equals(child.parent)) {
				collect(geometry, child, out);
			}
		}
	}

	/** A bone's pivot in Java model space. */
	private static float[] pivot(Geometry.Bone bone) {
		return new float[] {bone.pivot[0], MODEL_HEIGHT - bone.pivot[1], bone.pivot[2]};
	}

	/** Every drawn face of the bone's cubes, positions relative to the bone's pivot. */
	private static float[] quads(Geometry geometry, Geometry.Bone bone, float[] pivot) {
		int count = 0;
		for (Geometry.Cube c : bone.cubes) {
			for (float[] face : c.faces) {
				count += face == null ? 0 : 1;
			}
		}
		float[] out = new float[count * QUAD];
		int at = 0;
		for (Geometry.Cube c : bone.cubes) {
			at = cube(out, at, c, pivot, geometry.textureWidth, geometry.textureHeight);
		}
		return out;
	}

	/**
	 * The six faces' corners in the bone's Java space: min/max grown by the
	 * inflate, then turned by the cube's own rotation. Corners run top-left,
	 * top-right, bottom-right, bottom-left as seen from outside, matching the
	 * rectangle's corners.
	 */
	private static int cube(float[] out, int at, Geometry.Cube c, float[] pivot, float texW, float texH) {
		float x0 = c.origin[0] - pivot[0] - c.inflate;
		float y0 = MODEL_HEIGHT - c.origin[1] - c.size[1] - pivot[1] - c.inflate;
		float z0 = c.origin[2] - pivot[2] - c.inflate;
		float x1 = x0 + c.size[0] + 2 * c.inflate;
		float y1 = y0 + c.size[1] + 2 * c.inflate;
		float z1 = z0 + c.size[2] + 2 * c.inflate;
		float[][][] corners = {
			{{x0, y0, z0}, {x1, y0, z0}, {x1, y1, z0}, {x0, y1, z0}}, // north (-z, the front)
			{{x1, y0, z1}, {x0, y0, z1}, {x0, y1, z1}, {x1, y1, z1}}, // south
			{{x0, y0, z1}, {x0, y0, z0}, {x0, y1, z0}, {x0, y1, z1}}, // east (the player's right, -x)
			{{x1, y0, z0}, {x1, y0, z1}, {x1, y1, z1}, {x1, y1, z0}}, // west (+x)
			{{x0, y0, z1}, {x1, y0, z1}, {x1, y0, z0}, {x0, y0, z0}}, // up (-y in Java)
			{{x0, y1, z0}, {x1, y1, z0}, {x1, y1, z1}, {x0, y1, z1}}}; // down
		float[][] normals = {{0, 0, -1}, {0, 0, 1}, {-1, 0, 0}, {1, 0, 0}, {0, -1, 0}, {0, 1, 0}};
		float[] turn = null;
		float[] around = null;
		if (c.rotated()) {
			float rx = c.rotation[0] * DEG;
			float ry = c.rotation[1] * DEG;
			float rz = c.rotation[2] * DEG;
			float sx = (float) Math.sin(rx);
			float cx = (float) Math.cos(rx);
			float sy = (float) Math.sin(ry);
			float cy = (float) Math.cos(ry);
			float sz = (float) Math.sin(rz);
			float cz = (float) Math.cos(rz);
			turn = new float[] {cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx,
					sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx,
					-sy, cy * sx, cy * cx};
			float[] mid = {c.origin[0] + c.size[0] / 2, c.origin[1] + c.size[1] / 2, c.origin[2] + c.size[2] / 2};
			float[] p = c.pivot != null ? c.pivot : mid;
			around = new float[] {p[0] - pivot[0], MODEL_HEIGHT - p[1] - pivot[1], p[2] - pivot[2]};
		}
		for (int f = 0; f < Geometry.FACES; f++) {
			float[] rect = c.faces[f];
			if (rect == null) {
				continue;
			}
			float[][] uv = {{rect[0], rect[1]}, {rect[0] + rect[2], rect[1]}, {rect[0] + rect[2], rect[1] + rect[3]}, {rect[0], rect[1] + rect[3]}};
			for (int k = 0; k < 4; k++) {
				float[] p = corners[f][k];
				float x = p[0];
				float y = p[1];
				float z = p[2];
				if (turn != null) {
					float dx = x - around[0];
					float dy = y - around[1];
					float dz = z - around[2];
					x = around[0] + turn[0] * dx + turn[1] * dy + turn[2] * dz;
					y = around[1] + turn[3] * dx + turn[4] * dy + turn[5] * dz;
					z = around[2] + turn[6] * dx + turn[7] * dy + turn[8] * dz;
				}
				out[at++] = x;
				out[at++] = y;
				out[at++] = z;
				out[at++] = uv[k][0] / texW;
				out[at++] = uv[k][1] / texH;
			}
			float[] n = normals[f];
			if (turn != null) {
				n = new float[] {turn[0] * n[0] + turn[1] * n[1] + turn[2] * n[2], turn[3] * n[0] + turn[4] * n[1] + turn[5] * n[2],
						turn[6] * n[0] + turn[7] * n[1] + turn[8] * n[2]};
			}
			out[at++] = n[0];
			out[at++] = n[1];
			out[at++] = n[2];
		}
		return at;
	}
}
