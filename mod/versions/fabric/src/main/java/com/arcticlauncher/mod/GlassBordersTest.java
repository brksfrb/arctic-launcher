//#if MC >= 1.21.11 && MC < 26.1
package com.arcticlauncher.mod;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.core.BlockPos;
import net.minecraft.network.protocol.game.ClientboundLevelChunkWithLightPacket;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.phys.Vec3;

/**
 * Opt-in real-play chunk-border regression: -Pdevelopment_scenes=true -Darctic.selftest=ctmborders.
 * The modern twin of the 1.8.9 scene: a stained-glass ceiling and wall over many chunk borders, one ceiling
 * edge exactly on a chunk border; chunks arrive one at a time, then the player teleports away and back.
 * Measured from the masks the mesher really used: frames drawn while a neighbour chunk is still missing, and
 * the picture once everything has arrived (exactly what the loaded world gives).
 */
final class GlassBordersTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-ctm-borders"); t.setDaemon(true); return t;
	});
	private static final int R = 40, EDGE = 47, CEILING = 232, WALL_Z = -R - 1, WALL_LOW = 212;
	private static final ConcurrentHashMap<Long, Integer> seen = new ConcurrentHashMap<>();
	private static final Map<Long, Integer> settled = new HashMap<>();
	private static volatile boolean watch;
	private static int wrongSettled, framedArriving, framedBesideMissing;
	private GlassBordersTest() {}

	static void start() { later(10, () -> WorldTest.createTestWorld(() -> { command("tp @p 0.5 222 0.5 0 -90"); later(8, GlassBordersTest::build); })); }
	private static void later(double seconds, Runnable r) { TIMER.schedule(() -> Minecraft.getInstance().execute(r), (long) (seconds * 1000), TimeUnit.MILLISECONDS); }
	private static void command(String c) { Compat.sendChat("/" + c); }
	private static boolean ceiling(int x, int y, int z) { return y == CEILING && x >= -R && x <= EDGE && Math.abs(z) <= R; }
	private static boolean wall(int x, int y, int z) { return z == WALL_Z && x >= -R && x <= EDGE && y >= WALL_LOW && y < CEILING; }
	static boolean watching(BlockPos p) { return watch && (ceiling(p.getX(), p.getY(), p.getZ()) || wall(p.getX(), p.getY(), p.getZ())); }
	static void observe(BlockPos p, int masks) { seen.put(p.asLong(), masks); }

	private static void build() {
		Minecraft mc = Minecraft.getInstance();
		command("gamemode creative");
		for (int x = -R; x <= EDGE; x += 16) {
			int to = Math.min(EDGE, x + 15);
			command("fill " + x + " " + WALL_LOW + " " + WALL_Z + " " + to + " " + (CEILING + 6) + " " + R + " minecraft:air");
			command("fill " + x + " " + CEILING + " " + -R + " " + to + " " + CEILING + " " + R + " minecraft:light_blue_stained_glass");
			command("fill " + x + " " + WALL_LOW + " " + WALL_Z + " " + to + " " + (CEILING - 1) + " " + WALL_Z + " minecraft:blue_stained_glass");
		}
		command("fill " + (EDGE + 1) + " " + WALL_LOW + " " + WALL_Z + " " + (EDGE + 8) + " " + (CEILING + 6) + " " + R + " minecraft:air");
		command("fill -1 221 -1 1 221 1 minecraft:stone");
		command("time set 6000"); command("weather clear"); command("gamerule advance_time false");
		command("tp @p 0.5 222 0.5 0 -90");
		com.arcticlauncher.client.ArcticClient.config().connectedGlass = true;
		mc.options.hideGui = true;
		later(10, () -> {
			for (int x = -R; x <= EDGE; x++) {
				for (int z = -R; z <= R; z++) reference(new BlockPos(x, CEILING, z));
				for (int y = WALL_LOW; y < CEILING; y++) reference(new BlockPos(x, y, WALL_Z));
			}
			int edge = settled.get(new BlockPos(EDGE, CEILING, 0).asLong()) >> 8, inner = settled.get(new BlockPos(0, CEILING, 0).asLong()) >> 8;
			ArcticMod.LOG.info("ctmborders reference: {} blocks, inner mask {}, mask on the chunk-border edge {}", settled.size(), inner, edge);
			if (inner != 255 || edge == 255) throw new AssertionError("ctmborders: the reference is wrong");
			watch = true;
			mc.levelRenderer.allChanged();
			overview(() -> { verify("initial"); slow(); });
		});
	}
	private static void reference(BlockPos p) {
		Minecraft mc = Minecraft.getInstance();
		settled.put(p.asLong(), GlassCtm.masks(mc.level, mc.level.getBlockState(p), p));
	}

	private static void overview(Runnable then) { hold(0.5, 150, 0.5, System.nanoTime(), 10, then); }
	private static void place(double x, double y, double z) {
		Minecraft mc = Minecraft.getInstance();
		mc.player.getAbilities().flying = true;
		mc.player.setPos(x, y, z);
		mc.player.setDeltaMovement(Vec3.ZERO);
		mc.player.setXRot(-90);
	}
	private static void hold(double x, double y, double z, long start, double seconds, Runnable then) {
		if (Minecraft.getInstance().player == null) return;
		place(x, y, z);
		if ((System.nanoTime() - start) / 1e9 >= seconds) { then.run(); return; }
		later(0.02, () -> hold(x, y, z, start, seconds, then));
	}
	private static void wander(double cx, double cz, long start, double seconds, Runnable then) {
		if (Minecraft.getInstance().player == null) return;
		double t = (System.nanoTime() - start) / 1e9;
		if (t >= seconds) { place(cx, 222, cz); then.run(); return; }
		place(cx + Math.sin(t * 2.5) * 6, 222, cz + Math.cos(t * 1.7) * 6);
		later(0.02, () -> wander(cx, cz, start, seconds, then));
	}

	private static int[] differences(boolean log, String phase) {
		int framed = 0, joined = 0, meshed = 0;
		StringBuilder first = new StringBuilder();
		for (Map.Entry<Long, Integer> e : seen.entrySet()) {
			Integer want = settled.get(e.getKey());
			if (want == null) continue;
			meshed++;
			int got = e.getValue();
			if (got == want) continue;
			if ((got & want) != want) framed++; else joined++;
			if (log && framed + joined <= 10) { BlockPos p = BlockPos.of(e.getKey()); first.append(' ').append(p.getX()).append(',').append(p.getY()).append(',').append(p.getZ()).append('=').append(got >> 8).append('/').append(got & 255).append(" want ").append(want >> 8).append('/').append(want & 255); }
		}
		if (log) ArcticMod.LOG.info("ctmborders {} {}: meshed={} of {} framed={} wrongly joined={}{}", framed + joined == 0 && meshed > 0 ? "ok" : "WRONG", phase, meshed, settled.size(), framed, joined, first);
		return new int[]{framed, joined, meshed};
	}
	private static void verify(String phase) {
		int[] d = differences(true, phase);
		wrongSettled += d[0] + d[1] + (d[2] == settled.size() ? 0 : 1);
		WorldTest.shot(phase + "-settled");
	}
	private static void sample(String phase, long start, StringBuilder series, int[] peak, boolean[] shot) {
		double t = (System.nanoTime() - start) / 1e9;
		int framed = differences(false, phase)[0];
		if (framed != peak[2]) { peak[2] = framed; series.append(String.format(Locale.ROOT, " %.1fs:%d", t, framed)); }
		if (framed > 0) peak[1] = (int) (t * 1000);
		if (framed > peak[0]) peak[0] = framed;
		if (framed >= 24 && !shot[0]) { shot[0] = true; WorldTest.shot(phase + "-arriving"); }
		if (t < 12) { later(0.1, () -> sample(phase, start, series, peak, shot)); return; }
		framedArriving += peak[0];
		ArcticMod.LOG.info("ctmborders arrival {}: peak framed={} last at {}ms; series{}", phase, peak[0], peak[1], series);
	}
	private static void sample(String phase) { sample(phase, System.nanoTime(), new StringBuilder(), new int[3], new boolean[1]); }

	/** Chunks dropped on the client and delivered again one at a time as real packets, the outer columns much later. */
	private static void slow() {
		Minecraft mc = Minecraft.getInstance();
		List<ClientboundLevelChunkWithLightPacket> near = new ArrayList<>(), edge = new ArrayList<>();
		List<int[]> order = new ArrayList<>();
		for (int x = -4; x <= 4; x++) for (int z = -4; z <= 3; z++) order.add(new int[]{x, z});
		order.sort(java.util.Comparator.comparingInt(c -> (2 * c[0] + 1) * (2 * c[0] + 1) + (2 * c[1] + 1) * (2 * c[1] + 1)));
		command("tp @p 0.5 222 0.5 0 -90");
		later(3, () -> {
			for (int[] c : order) (c[0] >= 1 || c[0] < -2 ? edge : near).add(new ClientboundLevelChunkWithLightPacket(mc.level.getChunk(c[0], c[1]), mc.level.getChunkSource().getLightEngine(), null, null));
			seen.clear();
			for (int[] c : order) mc.level.getChunkSource().drop(new ChunkPos(c[0], c[1]));
			mc.levelRenderer.allChanged();
			ArcticMod.LOG.info("ctmborders slow: dropped {} chunks; {} arrive one per 120 ms, {} later", order.size(), near.size(), edge.size());
			sample("slow");
			wander(0.5, 0.5, System.nanoTime(), near.size() * 0.12 + 4, () -> { });
			deliver(near, 0, () -> later(10, () -> {
				besideMissing("slow-edge");
				hold(16.0, 222, 0.5, System.nanoTime(), 3, () -> {
					WorldTest.shot("slow-edge");
					deliver(edge, 0, () -> overview(() -> { verify("slow"); teleport(0); }));
				});
			}));
		});
	}
	private static void deliver(List<ClientboundLevelChunkWithLightPacket> packets, int index, Runnable then) {
		if (index == packets.size()) { then.run(); return; }
		Minecraft.getInstance().getConnection().handleLevelChunkWithLight(packets.get(index));
		later(0.12, () -> deliver(packets, index + 1, then));
	}
	private static void besideMissing(String phase) {
		int framed = 0, meshed = 0, inside = 0;
		for (int z = -R + 1; z < R; z++) for (int x : new int[]{15, -32}) {
			Integer m = seen.get(new BlockPos(x, CEILING, z).asLong());
			if (m == null) continue;
			meshed++;
			if (m >> 8 != 255) framed++;
		}
		for (int z = -R + 1; z < R; z++) for (int x = -31; x < 15; x++) { Integer m = seen.get(new BlockPos(x, CEILING, z).asLong()); if (m != null && m >> 8 != 255) inside++; }
		framedBesideMissing += framed + inside;
		ArcticMod.LOG.info("ctmborders {} {}: beside a missing chunk meshed={} framed={}; framed inside the loaded area={}", framed + inside == 0 ? "ok" : "FRAMED", phase, meshed, framed, inside);
	}

	private static final double[][] ARRIVALS = {{0.5, 0.5}, {15.5, 15.5}, {-16.5, 8.5}};
	private static void teleport(int round) {
		if (round == ARRIVALS.length) { finish(); return; }
		command("tp @p 3000.5 222 3000.5 0 -90");
		later(8, () -> {
			seen.clear();
			command("tp @p " + ARRIVALS[round][0] + " 222 " + ARRIVALS[round][1] + " 0 -90");
			sample("teleport" + round);
			later(0.5, () -> wander(ARRIVALS[round][0], ARRIVALS[round][1], System.nanoTime(), 9, () -> later(4, () -> overview(() -> { verify("teleport" + round); teleport(round + 1); }))));
		});
	}

	private static void finish() {
		boolean pass = wrongSettled == 0 && framedArriving == 0 && framedBesideMissing == 0;
		ArcticMod.LOG.info("ctmborders {}: wrong once settled={}, framed beside a missing chunk={}, framed at the arrival peaks={}", pass ? "PASS" : "FAIL", wrongSettled, framedBesideMissing, framedArriving);
		later(2, () -> Minecraft.getInstance().stop());
	}
}
//#endif
