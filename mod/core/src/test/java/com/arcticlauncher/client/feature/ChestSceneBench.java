package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.config.ClientConfig;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.OutputStreamWriter;
import java.io.Writer;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;
import org.junit.jupiter.api.Test;

/**
 * A storage hall with 70,000 chests and 35,000 hoppers, all within the game's 64-block draw distance of
 * the camera: what a frame costs the render thread the game's way, with block entity culling, and with
 * the chests drawn on the GPU ({@link ChestInstancing}). Writes build/reports/chest-scene-bench.txt.
 *
 * <p>Counted exactly: renderer calls, draws, vertices and bytes. Timed here: the culling test, the
 * instancing work, and a stand-in for the vertex work the game does per chest (72 vertices through a
 * matrix into a 36-byte vertex each, as its buffer builder does). That stand-in leaves out the render
 * state object, the model walk, the light and neighbour lookups and the draw sorting, so the game's
 * real cost is higher; it is there to give the before column a floor, not a measurement of the game.
 */
public class ChestSceneBench {
	static final int CHESTS = 70_000;
	static final int HOPPERS = 35_000;
	/** Vertices of a chest model (three boxes), and the bytes of one in the game's entity vertex format. */
	static final int VERTICES = 72;
	static final int VERTEX_BYTES = 36;

	static final class Chest {
		final int x;
		final int y;
		final int z;
		final int kind;
		ChestInstancing.Entry entry;

		Chest(int x, int y, int z, int kind) {
			this.x = x;
			this.y = y;
			this.z = z;
			this.kind = kind;
		}
	}

	static final class Hall implements ChestInstancing.World {
		@Override
		public boolean describe(ChestInstancing.Entry entry) {
			Chest chest = (Chest) entry.owner;
			entry.kind = chest.kind;
			entry.facing = (chest.x ^ chest.z) & 3;
			entry.sprite = 21 + chest.kind;
			entry.lightU = 0xE0;
			entry.lightV = 0;
			return true;
		}

		@Override
		public float open(ChestInstancing.Entry entry, float partialTick) {
			return 0.5f;
		}

		@Override
		public boolean shut(ChestInstancing.Entry entry) {
			return false;
		}

		@Override
		public void dropped(ChestInstancing.Entry entry) {}
	}

	/** Chests in an 84 x 40 x 84 hall around the camera (every corner inside 64 blocks), rows of double chests two apart. */
	static List<Chest> hall() {
		List<Chest> out = new ArrayList<Chest>(CHESTS);
		Random r = new Random(7);
		while (out.size() < CHESTS) {
			out.clear();
			for (int y = 50; y < 90; y++) {
				for (int z = -42; z < 42; z++) {
					for (int x = -42; x < 42 && out.size() < CHESTS; x += 2) {
						// Rows along x, every other row left free to walk (and for the hoppers below).
						if ((z & 1) == 0 && r.nextInt(100) < 99) {
							out.add(new Chest(x, y, z, 1));
							out.add(new Chest(x + 1, y, z, 2));
						}
					}
				}
			}
		}
		return out;
	}

	private static boolean sectionInView(Chest chest, SceneBench.Camera cam) {
		int sx = Math.floorDiv(chest.x, 16) * 16;
		int sy = Math.floorDiv(chest.y, 16) * 16;
		int sz = Math.floorDiv(chest.z, 16) * 16;
		return cam.sees(sx, sy, sz, sx + 16, sy + 16, sz + 16);
	}

	private static boolean inDistance(Chest chest, SceneBench.Camera cam) {
		double dx = chest.x + 0.5 - cam.x;
		double dy = chest.y + 0.5 - cam.y;
		double dz = chest.z + 0.5 - cam.z;
		return dx * dx + dy * dy + dz * dz < 64 * 64;
	}

	/** The stand-in for the game's vertex work for one chest: 72 vertices placed by a matrix and written out. */
	private static void buildVertices(ByteBuffer out, float[] m, float x, float y, float z) {
		for (int v = 0; v < VERTICES; v++) {
			float lx = (v & 1) * 0.875f + 0.0625f;
			float ly = ((v >> 1) & 1) * 0.625f;
			float lz = ((v >> 2) & 1) * 0.875f + 0.0625f;
			out.putFloat(m[0] * lx + m[4] * ly + m[8] * lz + m[12] + x);
			out.putFloat(m[1] * lx + m[5] * ly + m[9] * lz + m[13] + y);
			out.putFloat(m[2] * lx + m[6] * ly + m[10] * lz + m[14] + z);
			out.putInt(-1);
			out.putFloat(lx * 0.25f).putFloat(lz * 0.25f);
			out.putInt(0x000A0000).putInt(0x00F000E0);
			out.put((byte) (m[4] * 127)).put((byte) (m[5] * 127)).put((byte) (m[6] * 127)).put((byte) 0);
		}
	}

	@Test
	void seventyThousandChests() throws IOException {
		List<Chest> chests = hall();
		assertEquals(CHESTS, chests.size());
		ClientConfig config = new ClientConfig();
		Random r = new Random(42);
		List<SceneBench.Camera> views = new ArrayList<SceneBench.Camera>();
		for (int i = 0; i < 48; i++) {
			views.add(new SceneBench.Camera(r.nextDouble() * 4 - 2, 70.62, r.nextDouble() * 4 - 2 + 0.5, r.nextDouble() * 360, r.nextDouble() * 50 - 25));
		}

		// Renderer calls: the game's (every chest of a section in view, within 64 blocks), and with block entity culling.
		long gameCalls = 0;
		long culledCalls = 0;
		long worstGame = 0;
		long worstCulled = 0;
		for (SceneBench.Camera cam : views) {
			long g = 0;
			long c = 0;
			for (Chest chest : chests) {
				if (sectionInView(chest, cam) && inDistance(chest, cam)) {
					g++;
					if (!BlockEntityCulling.skip(config, chest.x, chest.y, chest.z, cam.x, cam.y, cam.z, cam)) {
						c++;
					}
				}
			}
			gameCalls += g;
			culledCalls += c;
			worstGame = Math.max(worstGame, g);
			worstCulled = Math.max(worstCulled, c);
		}
		double game = gameCalls / (double) views.size();
		double culled = culledCalls / (double) views.size();

		// The culling test's cost per chest in a section in view.
		long sink = 0;
		long tested = 0;
		long start = 0;
		for (int round = 0; round < 5; round++) {
			if (round == 2) {
				start = System.nanoTime();
				tested = 0;
			}
			for (SceneBench.Camera cam : views) {
				for (Chest chest : chests) {
					tested++;
					sink += BlockEntityCulling.skip(config, chest.x, chest.y, chest.z, cam.x, cam.y, cam.z, cam) ? 1 : 0;
				}
			}
		}
		double nsCull = (System.nanoTime() - start) / (double) tested;

		// The stand-in for the game's vertex work per chest.
		ByteBuffer vertexBytes = ByteBuffer.allocateDirect(CHESTS * VERTICES * VERTEX_BYTES).order(ByteOrder.nativeOrder());
		float[] matrix = {0, 0, -1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0.5f, 0, 0.5f, 1};
		long built = 0;
		for (int round = 0; round < 8; round++) {
			if (round == 3) {
				start = System.nanoTime();
				built = 0;
			}
			vertexBytes.clear();
			for (Chest chest : chests) {
				buildVertices(vertexBytes, matrix, chest.x, chest.y - 70.62f, chest.z);
				built++;
			}
		}
		double nsVertices = (System.nanoTime() - start) / (double) built;
		sink += vertexBytes.get(123);

		// On the GPU: every chest kept; each frame the game hands over the chests of the sections in view.
		ChestInstancing instancing = new ChestInstancing();
		Hall hallWorld = new Hall();
		for (Chest chest : chests) {
			chest.entry = instancing.add(chest, chest.x, chest.y, chest.z, false);
		}
		List<List<Chest>> seenPerView = new ArrayList<List<Chest>>();
		for (SceneBench.Camera cam : views) {
			List<Chest> seen = new ArrayList<Chest>();
			for (Chest chest : chests) {
				if (sectionInView(chest, cam)) {
					seen.add(chest);
				}
			}
			seenPerView.add(seen);
		}
		// First sight of everything: every section is written once.
		long firstStart = System.nanoTime();
		instancing.beginFrame(0.5, 70.62, 0.5, 64);
		for (Chest chest : chests) {
			instancing.see(chest.entry, false);
		}
		ChestInstancing.Frame frame = instancing.endFrame(hallWorld, 0f);
		double firstMs = (System.nanoTime() - firstStart) / 1e6;
		assertEquals(CHESTS, frame.total());
		long bakedFloats = instancing.bakedFloats();

		// The camera turning: another set of sections every frame (the worst case: everything is copied again).
		double nsTurning = 0;
		long instancesTurning = 0;
		long copied = 0;
		for (int round = 0; round < 12; round++) {
			if (round == 4) {
				start = System.nanoTime();
				instancesTurning = 0;
				instancing.resetCounters();
			}
			for (int v = 0; v < views.size(); v++) {
				SceneBench.Camera cam = views.get(v);
				instancing.beginFrame(cam.x, cam.y, cam.z, 64);
				List<Chest> seen = seenPerView.get(v);
				for (int i = 0; i < seen.size(); i++) {
					instancing.see(seen.get(i).entry, false);
				}
				frame = instancing.endFrame(hallWorld, 0f);
				instancesTurning += frame.total();
			}
		}
		nsTurning = (System.nanoTime() - start) / (8.0 * views.size());
		copied = instancing.copiedFloats / (8L * views.size());
		double turningInstances = instancesTurning / (8.0 * views.size());

		// The camera still: the same sections every frame; nothing is copied or uploaded.
		SceneBench.Camera still = views.get(0);
		List<Chest> stillSeen = seenPerView.get(0);
		long stillFrames = 0;
		for (int round = 0; round < 600; round++) {
			if (round == 200) {
				start = System.nanoTime();
				stillFrames = 0;
				instancing.resetCounters();
			}
			instancing.beginFrame(still.x, still.y, still.z, 64);
			for (int i = 0; i < stillSeen.size(); i++) {
				instancing.see(stillSeen.get(i).entry, false);
			}
			frame = instancing.endFrame(hallWorld, 0f);
			stillFrames++;
		}
		double nsStill = (System.nanoTime() - start) / (double) stillFrames;
		assertEquals(0, instancing.uploads, "a still view uploads nothing");
		assertEquals(0, instancing.bakes);
		int stillInstances = frame.total();

		// Ten chests open at once: they alone are worked out each frame (the buffer is copied again).
		for (int i = 0; i < 10; i++) {
			instancing.wake(stillSeen.get(i * 97).entry);
		}
		long movingFrames = 0;
		for (int round = 0; round < 300; round++) {
			if (round == 100) {
				start = System.nanoTime();
				movingFrames = 0;
			}
			instancing.beginFrame(still.x, still.y, still.z, 64);
			for (int i = 0; i < stillSeen.size(); i++) {
				instancing.see(stillSeen.get(i).entry, false);
			}
			frame = instancing.endFrame(hallWorld, 0f);
			movingFrames++;
		}
		double nsMoving = (System.nanoTime() - start) / (double) movingFrames;
		assertEquals(stillInstances, frame.total());

		double frameBytes = (ChestInstancing.SPRITES * 4 + CHESTS * (double) ChestInstancing.FLOATS) * 4;
		StringBuilder out = new StringBuilder();
		out.append(String.format("Storage hall: %,d chests (double chests) and %,d hoppers within 64 blocks of the camera; %d views from its middle%n", CHESTS, HOPPERS,
				views.size()));
		out.append(String.format("hoppers: no renderer and no client tick on 26.2: 0 calls, 0 draws, 0 ns in every column%n%n"));
		out.append(String.format("%-46s %16s %16s %16s%n", "per frame", "game (main)", "+ culling", "+ GPU chests"));
		out.append(String.format("%-46s %,16.0f %,16.0f %16s%n", "chest renderer calls (average)", game, culled, "0"));
		out.append(String.format("%-46s %,16d %,16d %16s%n", "chest renderer calls (worst view)", worstGame, worstCulled, "0"));
		out.append(String.format("%-46s %,16.0f %,16.0f %16s%n", "render states made", game, culled, "0"));
		out.append(String.format("%-46s %,16.0f %,16.0f %16s%n", "vertices built on the CPU", game * VERTICES, culled * VERTICES, "0"));
		out.append(String.format("%-46s %,16.1f %,16.1f %16s%n", "vertex data built and uploaded (MB)", game * VERTICES * VERTEX_BYTES / 1e6,
				culled * VERTICES * VERTEX_BYTES / 1e6, "0 (view still)"));
		out.append(String.format("%-46s %16s %16s %16s%n", "chest draw calls", "1", "1", "3 (instanced)"));
		out.append(String.format("%-46s %,16.1f %,16.1f %16s%n", "CPU ms, vertex work alone (stand-in, floor)", game * nsVertices / 1e6,
				(culled * nsVertices + game * nsCull) / 1e6, "-"));
		out.append(String.format("%-46s %16s %16s %,16.2f%n", "CPU ms, view still (measured)", "-", "-", nsStill / 1e6));
		out.append(String.format("%-46s %16s %16s %,16.2f%n", "CPU ms, view turning every frame (measured)", "-", "-", nsTurning / 1e6));
		out.append(String.format("%-46s %16s %16s %,16.2f%n", "CPU ms, ten lids moving (measured)", "-", "-", nsMoving / 1e6));
		out.append(String.format("%n+ GPU chests, details:%n"));
		out.append(String.format("  instances drawn: %,d with the view still, %,.0f on average while turning (the GPU clips what is off screen)%n", stillInstances,
				turningInstances));
		out.append(String.format("  instance data uploaded per frame: 0 MB with the view still, %.2f MB while turning or while a lid moves%n", copied * 4 / 1e6));
		out.append(String.format("  first sight of all %,d chests (every section written once): %.1f ms%n", CHESTS, firstMs));
		out.append(String.format("  memory: %.2f MB kept per section on the CPU, %.2f MB frame buffer on the CPU, up to %.2f MB on the GPU (three buffers in turn, with headroom), "
				+ "%.1f KB of models%n", bakedFloats * 4 / 1e6, frameBytes / 1e6, 3 * 1.5 * frameBytes / 1e6, 3 * VERTICES * 36 / 1e3));
		out.append(String.format("  for scale, the game's way: %.1f MB of vertices built and uploaded every frame%n", game * VERTICES * VERTEX_BYTES / 1e6));
		out.append(String.format("measured here: culling test %.1f ns per chest; vertex stand-in %.0f ns per chest (sink %d)%n", nsCull, nsVertices, sink));
		System.out.print(out);
		File report = new File("build/reports/chest-scene-bench.txt");
		report.getParentFile().mkdirs();
		Writer w = new OutputStreamWriter(new FileOutputStream(report), StandardCharsets.UTF_8);
		try {
			w.write(out.toString());
		} finally {
			w.close();
		}

		assertTrue(culled < game, "culling drops chests outside the view");
		assertTrue(nsStill < 5e6, "a still frame with 70,000 chests takes well under 5 ms of instancing work: " + nsStill / 1e6);
	}
}
