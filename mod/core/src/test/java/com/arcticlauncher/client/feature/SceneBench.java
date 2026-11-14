package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.config.ClientConfig;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.OutputStreamWriter;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;
import org.junit.jupiter.api.Test;

/**
 * A storage base with 500 chests, 300 hoppers, signs, ender chests and shulker boxes, seen from 400
 * places and directions inside it: how many block entity renderer calls a frame makes with the game's
 * section culling alone, and with block entity culling on top. Writes build/reports/block-entity-bench.txt.
 *
 * <p>The game's frustum test is stood in for by a plain six-plane frustum (70 degree field of view,
 * 16:9); the game's section visibility graph (which also drops sections behind walls) is left out, so
 * both counts are an upper bound and the difference is what the per-block-entity test removes.
 */
public class SceneBench {
	enum Kind {
		CHEST(true), SIGN(true), ENDER_CHEST(true), SHULKER(true),
		/** No renderer: the game never puts hoppers (or furnaces, barrels, droppers) in a section's list of block entities to draw. */
		HOPPER(false), BARREL(false), FURNACE(false);

		final boolean hasRenderer;

		Kind(boolean hasRenderer) {
			this.hasRenderer = hasRenderer;
		}
	}

	static final class BlockEntity {
		final Kind kind;
		final int x;
		final int y;
		final int z;

		BlockEntity(Kind kind, int x, int y, int z) {
			this.kind = kind;
			this.x = x;
			this.y = y;
			this.z = z;
		}
	}

	/** A camera with a six-plane frustum, like the game's. */
	static final class Camera implements BlockEntityCulling.View {
		final double x;
		final double y;
		final double z;
		/** Plane normals (pointing inside) and offsets: n·p + d >= 0 inside. */
		final double[][] planes = new double[6][4];

		Camera(double x, double y, double z, double yawDeg, double pitchDeg) {
			this.x = x;
			this.y = y;
			this.z = z;
			double yaw = Math.toRadians(yawDeg);
			double pitch = Math.toRadians(pitchDeg);
			double[] f = {-Math.sin(yaw) * Math.cos(pitch), -Math.sin(pitch), Math.cos(yaw) * Math.cos(pitch)};
			double[] r = {Math.cos(yaw), 0, Math.sin(yaw)};
			double[] u = cross(r, f);
			double tanV = Math.tan(Math.toRadians(70) / 2);
			double tanH = tanV * 16 / 9;
			plane(0, f, 0.05);
			// A point p (from the camera) is inside when |p·r| <= tanH (p·f) and |p·u| <= tanV (p·f).
			plane(1, edge(f, tanH, r, -1), 0);
			plane(2, edge(f, tanH, r, 1), 0);
			plane(3, edge(f, tanV, u, -1), 0);
			plane(4, edge(f, tanV, u, 1), 0);
			plane(5, new double[] {-f[0], -f[1], -f[2]}, -512);
		}

		static Camera random(Random r) {
			return new Camera(r.nextDouble() * 50 - 25, 65.62, r.nextDouble() * 40 - 20, r.nextDouble() * 360, r.nextDouble() * 60 - 30);
		}

		private static double[] cross(double[] a, double[] b) {
			return new double[] {a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]};
		}

		/** The inward normal of a side plane: f·tan + sign·side. */
		private static double[] edge(double[] f, double tan, double[] side, double sign) {
			return new double[] {f[0] * tan + sign * side[0], f[1] * tan + sign * side[1], f[2] * tan + sign * side[2]};
		}

		private void plane(int i, double[] n, double offset) {
			// Inside: n·(p - cam) - offset >= 0.
			planes[i][0] = n[0];
			planes[i][1] = n[1];
			planes[i][2] = n[2];
			planes[i][3] = -(n[0] * x + n[1] * y + n[2] * z) - offset;
		}

		@Override
		public boolean sees(double minX, double minY, double minZ, double maxX, double maxY, double maxZ) {
			for (double[] p : planes) {
				// The box corner farthest along the plane's normal.
				double px = p[0] >= 0 ? maxX : minX;
				double py = p[1] >= 0 ? maxY : minY;
				double pz = p[2] >= 0 ? maxZ : minZ;
				if (p[0] * px + p[1] * py + p[2] * pz + p[3] < 0) {
					return false;
				}
			}
			return true;
		}
	}

	static List<BlockEntity> base() {
		List<BlockEntity> out = new ArrayList<BlockEntity>();
		// Five walls of chests, two high and fifty long: 500 chests (250 double chests), and a sign on each lower double chest.
		for (int wall = 0; wall < 5; wall++) {
			int z = -20 + wall * 10;
			for (int x = -25; x < 25; x++) {
				out.add(new BlockEntity(Kind.CHEST, x, 64, z));
				out.add(new BlockEntity(Kind.CHEST, x, 65, z));
				if ((x & 1) == 0) {
					out.add(new BlockEntity(Kind.SIGN, x, 64, z - 1));
				}
			}
		}
		// 300 hoppers feeding them from below (the sorter), and a smelter row.
		for (int i = 0; i < 300; i++) {
			out.add(new BlockEntity(Kind.HOPPER, -25 + i % 50, 63, -20 + (i / 50) * 10 - (i / 250)));
		}
		for (int i = 0; i < 24; i++) {
			out.add(new BlockEntity(Kind.FURNACE, -12 + i, 64, 25));
			out.add(new BlockEntity(Kind.BARREL, -12 + i, 66, 25));
		}
		Random r = new Random(1);
		for (int i = 0; i < 20; i++) {
			out.add(new BlockEntity(Kind.ENDER_CHEST, r.nextInt(50) - 25, 64, r.nextInt(8) - 4 + 5));
		}
		for (int i = 0; i < 40; i++) {
			out.add(new BlockEntity(Kind.SHULKER, r.nextInt(50) - 25, 66, r.nextInt(40) - 20));
		}
		return out;
	}

	/** Renderer calls the game makes: block entities with a renderer, in a section in view, within its 64 blocks. */
	static boolean gameDraws(BlockEntity e, Camera cam) {
		if (!e.kind.hasRenderer) {
			return false;
		}
		int sx = Math.floorDiv(e.x, 16) * 16;
		int sy = Math.floorDiv(e.y, 16) * 16;
		int sz = Math.floorDiv(e.z, 16) * 16;
		if (!cam.sees(sx, sy, sz, sx + 16, sy + 16, sz + 16)) {
			return false;
		}
		double dx = e.x + 0.5 - cam.x;
		double dy = e.y + 0.5 - cam.y;
		double dz = e.z + 0.5 - cam.z;
		return dx * dx + dy * dy + dz * dz < 64 * 64;
	}

	@Test
	void storageBase() throws IOException {
		List<BlockEntity> scene = base();
		int chests = 0;
		int hoppers = 0;
		for (BlockEntity e : scene) {
			chests += e.kind == Kind.CHEST ? 1 : 0;
			hoppers += e.kind == Kind.HOPPER ? 1 : 0;
		}
		assertEquals(500, chests);
		assertEquals(300, hoppers);

		ClientConfig on = new ClientConfig();
		Random r = new Random(42);
		int frames = 400;
		long before = 0;
		long after = 0;
		long[] beforeKind = new long[Kind.values().length];
		long[] afterKind = new long[Kind.values().length];
		long worstBefore = 0;
		long worstAfter = 0;
		List<Camera> cams = new ArrayList<Camera>();
		for (int f = 0; f < frames; f++) {
			Camera cam = Camera.random(r);
			cams.add(cam);
			long b = 0;
			long a = 0;
			for (BlockEntity e : scene) {
				if (!gameDraws(e, cam)) {
					continue;
				}
				b++;
				beforeKind[e.kind.ordinal()]++;
				if (!BlockEntityCulling.skip(on, e.x, e.y, e.z, cam.x, cam.y, cam.z, cam)) {
					a++;
					afterKind[e.kind.ordinal()]++;
				}
			}
			before += b;
			after += a;
			worstBefore = Math.max(worstBefore, b);
			worstAfter = Math.max(worstAfter, a);
		}

		// A shorter draw distance (the player's choice): 32 blocks.
		ClientConfig near = new ClientConfig();
		near.blockEntityDistance = 32;
		long after32 = 0;
		for (Camera cam : cams) {
			for (BlockEntity e : scene) {
				if (gameDraws(e, cam) && !BlockEntityCulling.skip(near, e.x, e.y, e.z, cam.x, cam.y, cam.z, cam)) {
					after32++;
				}
			}
		}

		// What the test itself costs, per block entity the game would draw.
		long tested = 0;
		long sink = 0;
		long start = 0;
		for (int round = 0; round < 6; round++) {
			if (round == 2) {
				start = System.nanoTime();
				tested = 0;
			}
			for (Camera cam : cams) {
				for (BlockEntity e : scene) {
					if (e.kind.hasRenderer) {
						tested++;
						sink += BlockEntityCulling.skip(on, e.x, e.y, e.z, cam.x, cam.y, cam.z, cam) ? 1 : 0;
					}
				}
			}
		}
		double nsPerTest = (System.nanoTime() - start) / (double) tested;

		StringBuilder out = new StringBuilder();
		out.append("Block entity culling, storage base: ").append(scene.size()).append(" block entities (500 chests, 300 hoppers, 125 signs, 20 ender chests, 40 shulker boxes, 24 furnaces, 24 barrels), ")
				.append(frames).append(" camera views inside it\n");
		out.append(String.format("renderer calls per frame, game (section culling):      %.1f average, %d worst%n", before / (double) frames, worstBefore));
		out.append(String.format("renderer calls per frame, with block entity culling:  %.1f average, %d worst (%.0f%% fewer)%n", after / (double) frames, worstAfter,
				100.0 * (before - after) / before));
		out.append(String.format("  ... and a 32-block draw distance:                  %.1f average%n", after32 / (double) frames));
		for (Kind k : Kind.values()) {
			out.append(String.format("  %-12s %7.1f -> %7.1f%n", k.name().toLowerCase(), beforeKind[k.ordinal()] / (double) frames, afterKind[k.ordinal()] / (double) frames));
		}
		out.append(String.format("cost of the test: %.1f ns per block entity (sink %d)%n", nsPerTest, sink));
		System.out.print(out);
		File report = new File("build/reports/block-entity-bench.txt");
		report.getParentFile().mkdirs();
		Writer w = new OutputStreamWriter(new FileOutputStream(report), StandardCharsets.UTF_8);
		try {
			w.write(out.toString());
		} finally {
			w.close();
		}

		assertEquals(0, beforeKind[Kind.HOPPER.ordinal()], "hoppers have no renderer");
		assertTrue(after < before * 0.8, "block entity culling should drop at least a fifth of the calls here");
		assertTrue(after32 <= after);
	}
}
