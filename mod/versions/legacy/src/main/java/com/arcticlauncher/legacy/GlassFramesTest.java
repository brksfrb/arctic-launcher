package com.arcticlauncher.legacy;

import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.block.BlockState;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;

/**
 * Opt-in real-play scene: -Pdevelopment_scenes=true -Darctic.selftest=ctmframes.
 * Glass seen from below in the dark: ceilings on and across a section boundary (y = 224), mixed plain and
 * stained glass with a pane row, pillars on a ceiling, a solid mass with shafts, stacked floors, glass under
 * lamps, and a ceiling placed block by block. Every face the mesher drew is compared with what the loaded
 * world gives, so a block that keeps its own frame inside a joined area is counted, not only photographed.
 */
final class GlassFramesTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-ctm-frames"); t.setDaemon(true); return t;
	});
	private static final String BLUE = "minecraft:stained_glass 11";
	private static final String[] NAMES = {"ceilings", "mixed", "pillars", "mass", "floors", "lit", "placed", "layers", "columns"};
	private static final int[][] CENTRES = {{-22, -11}, {0, -11}, {22, -11}, {-22, 11}, {0, 11}, {22, 11}, {0, 30}, {-22, 31}, {22, 31}};
	/** Last mask the mesher used per block face. */
	private static final ConcurrentHashMap<Long, Integer> seen = new ConcurrentHashMap<>();
	private static volatile boolean watch;
	private static int wrong;
	private GlassFramesTest() {}

	static void start() { later(10, () -> LegacyWorldTest.createTestWorld(() -> { command("tp @p 0.5 208 0.5 0 -90"); later(8, GlassFramesTest::build); })); }
	private static void later(double seconds, Runnable r) { TIMER.schedule(() -> LegacyHooks.onNextFrame(r), (long) (seconds * 1000), TimeUnit.MILLISECONDS); }
	private static void command(String c) { LegacyWorldTest.command(c); }
	private static void fill(int x1, int y1, int z1, int x2, int y2, int z2, String block) { command("fill " + x1 + " " + y1 + " " + z1 + " " + x2 + " " + y2 + " " + z2 + " " + block); }
	private static boolean inside(int x, int y, int z) { return x >= -40 && x <= 40 && z >= -30 && z <= 45 && y >= 205 && y <= 245; }
	private static long key(int x, int y, int z, int face) { return new BlockPos(x, y, z).asLong() * 8 + face; }
	/** A block is meshed again: the faces it drew before are gone. */
	static void meshing(int x, int y, int z) { if (watch && inside(x, y, z)) for (int f = 0; f < 6; f++) seen.remove(key(x, y, z, f)); }
	static void observe(int x, int y, int z, int face, int mask) { if (watch && inside(x, y, z)) seen.put(key(x, y, z, face), mask); }

	private static void build() {
		MinecraftClient mc = MinecraftClient.getInstance();
		command("gamemode 1");
		int x = CENTRES[0][0], z = CENTRES[0][1];
		fill(x - 4, 222, z - 4, x - 2, 222, z + 4, BLUE);
		fill(x - 1, 223, z - 4, x + 1, 224, z + 4, BLUE);
		fill(x + 2, 224, z - 4, x + 4, 224, z + 4, BLUE);
		x = CENTRES[1][0]; z = CENTRES[1][1];
		fill(x - 4, 222, z - 4, x + 4, 222, z + 4, BLUE);
		fill(x - 4, 222, z - 2, x + 4, 222, z - 1, "minecraft:glass");
		fill(x - 4, 222, z, x + 4, 222, z, "minecraft:stained_glass 3");
		fill(x - 4, 222, z + 2, x + 4, 222, z + 2, "minecraft:stained_glass_pane 11");
		x = CENTRES[2][0]; z = CENTRES[2][1];
		fill(x - 4, 222, z - 4, x + 4, 222, z + 4, BLUE);
		for (int dx = -4; dx <= 4; dx += 2) for (int dz = -4; dz <= 4; dz += 2) fill(x + dx, 223, z + dz, x + dx, 234, z + dz, BLUE);
		x = CENTRES[3][0]; z = CENTRES[3][1];
		fill(x - 3, 218, z - 3, x + 3, 228, z + 3, BLUE);
		for (int dx = -2; dx <= 2; dx += 2) for (int dz = -2; dz <= 2; dz += 2) fill(x + dx, 218, z + dz, x + dx, 228, z + dz, "minecraft:air");
		x = CENTRES[4][0]; z = CENTRES[4][1];
		for (int y = 216; y <= 226; y += 2) fill(x - 4, y, z - 4, x + 4, y, z + 4, BLUE);
		x = CENTRES[5][0]; z = CENTRES[5][1];
		fill(x - 4, 223, z - 4, x + 4, 223, z + 4, BLUE);
		fill(x - 4, 226, z - 4, x + 4, 226, z + 4, BLUE);
		for (int dx = -2; dx <= 2; dx += 4) for (int dz = -2; dz <= 2; dz += 4) command("setblock " + (x + dx) + " 224 " + (z + dz) + " minecraft:sea_lantern");
		// Burak's plot: every level of the build is another colour (purple, blue, purple, ...), with 1x1 shafts
		// in one copy and 1x1 columns of a third colour in the other.
		for (int copy = 7; copy <= 8; copy++) {
			x = CENTRES[copy][0]; z = CENTRES[copy][1];
			for (int y = 216; y <= 231; y++) fill(x - 4, y, z - 4, x + 4, y, z + 4, "minecraft:stained_glass " + (y % 2 == 0 ? 10 : 11));
			for (int dx = -2; dx <= 2; dx += 2) for (int dz = -2; dz <= 2; dz += 2) fill(x + dx, 216, z + dz, x + dx, 231, z + dz, copy == 7 ? "minecraft:air" : "minecraft:stained_glass 3");
		}
		command("time set 18000"); command("weather clear"); command("gamerule doDaylightCycle false");
		com.arcticlauncher.client.ArcticClient.config().connectedGlass = true;
		mc.options.hudHidden = true;
		watch = true;
		later(3, () -> place(0));
	}

	/** A 7x7 ceiling placed one block at a time in a scattered order, as a builder or a server would. */
	private static void place(int i) {
		int x = CENTRES[6][0], z = CENTRES[6][1];
		for (int n = 0; n < 7 && i < 49; n++, i++) {
			int cell = i * 20 % 49;
			command("setblock " + (x - 3 + cell % 7) + " 223 " + (z - 3 + cell / 7) + " " + BLUE);
		}
		int next = i;
		if (i < 49) later(0.25, () -> place(next)); else later(4, () -> tour(0, true));
	}

	private static void tour(int scene, boolean night) {
		if (scene == CENTRES.length) {
			if (night) { command("time set 6000"); later(3, () -> tour(0, false)); return; }
			ArcticLegacy.LOG.info("ctmframes {}: {} faces wrong in total", wrong == 0 ? "PASS" : "FAIL", wrong);
			later(2, () -> MinecraftClient.getInstance().scheduleStop());
			return;
		}
		String label = NAMES[scene] + (night ? "-night" : "-day");
		double x = CENTRES[scene][0] + 0.5, z = CENTRES[scene][1] + 0.5;
		hold(x, 209, z, 0, -90, System.nanoTime(), 5, () -> {
			if (night) verify(scene);
			LegacyWorldTest.shot(label + "-below");
			later(0.5, () -> hold(x, 212, z - 9, 0, -42, System.nanoTime(), 4, () -> {
				LegacyWorldTest.shot(label + "-oblique");
				later(0.5, () -> tour(scene + 1, night));
			}));
		});
	}

	private static void hold(double x, double y, double z, float yaw, float pitch, long start, double seconds, Runnable then) {
		MinecraftClient mc = MinecraftClient.getInstance();
		if (mc.player == null) return;
		mc.player.abilities.flying = true;
		mc.player.updatePosition(x, y, z);
		mc.player.velocityX = mc.player.velocityY = mc.player.velocityZ = 0;
		mc.player.yaw = yaw; mc.player.pitch = pitch;
		if ((System.nanoTime() - start) / 1e9 >= seconds) { then.run(); return; }
		later(0.02, () -> hold(x, y, z, yaw, pitch, start, seconds, then));
	}

	/** Every face the mesher drew around this scene must carry the mask the loaded world gives. */
	private static void verify(int scene) {
		MinecraftClient mc = MinecraftClient.getInstance();
		int cx = CENTRES[scene][0], cz = CENTRES[scene][1], faces = 0, framed = 0, joined = 0, blocks = 0, lone = 0;
		StringBuilder first = new StringBuilder();
		Map<Long, Integer> copy = new HashMap<>(seen);
		Direction[] directions = Direction.values();
		for (int x = cx - 6; x <= cx + 6; x++) for (int z = cz - 6; z <= cz + 6; z++) for (int y = 210; y <= 240; y++) {
			BlockPos p = new BlockPos(x, y, z);
			BlockState state = mc.world.getBlockState(p);
			boolean any = false;
			for (int f = 0; f < 6; f++) {
				Integer got = copy.get(key(x, y, z, f));
				if (got == null) continue;
				any = true; faces++;
				watch = false;
				int want = LegacyGlassCtm.mask(mc.world, state, p, directions[f]);
				watch = true;
				if (got == want) continue;
				if ((got & want) != want) framed++; else joined++;
				if (got == 0 && want != 0) lone++;
				if (framed + joined <= 8) first.append(' ').append(x).append(',').append(y).append(',').append(z).append(' ').append(directions[f]).append('=').append(got).append(" want ").append(want);
			}
			if (any) blocks++;
		}
		wrong += framed + joined + (faces == 0 ? 1 : 0);
		ArcticLegacy.LOG.info("ctmframes {} {}: blocks={} faces={} framed={} (own full frame={}) wrongly joined={}{}", framed + joined == 0 && faces > 0 ? "ok" : "WRONG", NAMES[scene], blocks, faces, framed, lone, joined, first);
	}

	/**
	 * -Darctic.selftest=ctmlines: the "Lines between blocks" slider, changed while in the world. Each level is
	 * set as the slider sets it; the picture must follow without a restart. Clear glass, its panes, and stained
	 * glass beside them (which the slider leaves alone); then the settings page with its preview.
	 */
	static void startLines() { later(10, () -> LegacyWorldTest.createTestWorld(() -> { command("tp @p 0.5 220 -2.5 0 0"); later(8, GlassFramesTest::buildLines); })); }
	private static void buildLines() {
		MinecraftClient mc = MinecraftClient.getInstance();
		command("gamemode 1");
		fill(-12, 219, -8, 12, 219, 12, "minecraft:stone");
		fill(-12, 220, -8, 12, 232, 12, "minecraft:air");
		fill(-9, 220, 9, 9, 226, 9, "minecraft:bookshelf");
		fill(-4, 220, 3, 0, 223, 3, "minecraft:glass");
		fill(2, 220, 3, 5, 222, 3, "minecraft:glass_pane");
		fill(-8, 220, 3, -6, 222, 3, "minecraft:stained_glass 3");
		fill(-3, 220, 0, -1, 220, 1, "minecraft:glass");
		command("time set 6000"); command("weather clear"); command("gamerule doDaylightCycle false");
		com.arcticlauncher.client.ArcticClient.config().connectedGlass = true;
		mc.options.hudHidden = true;
		later(4, () -> lines(new int[] {0, 25, 50, 100}, 0));
	}
	private static void lines(int[] levels, int i) {
		MinecraftClient mc = MinecraftClient.getInstance();
		if (i == levels.length) { later(1, () -> MinecraftClient.getInstance().scheduleStop()); return; }
		com.arcticlauncher.client.ArcticClient.config().clearGlassLines = levels[i];
		hold(-0.5, 220.4, -2.2, 0, 2, System.nanoTime(), 9, () -> {
			ArcticLegacy.LOG.info("ctmlines level {}: tiles stitched at {} ({})", levels[i], com.arcticlauncher.client.ctm.GlassLines.stitched(),
					com.arcticlauncher.client.ctm.GlassLines.stitched() == levels[i] ? "ok" : "WRONG");
			LegacyWorldTest.shot("lines-" + levels[i] + "-world");
			later(0.5, () -> {
				mc.options.hudHidden = false;
				com.arcticlauncher.client.menu.Menus.open("connected-glass");
				later(2, () -> {
					LegacyWorldTest.shot("lines-" + levels[i] + "-settings");
					later(0.5, () -> { mc.setScreen(null); mc.options.hudHidden = true; later(0.5, () -> lines(levels, i + 1)); });
				});
			});
		});
	}
}
