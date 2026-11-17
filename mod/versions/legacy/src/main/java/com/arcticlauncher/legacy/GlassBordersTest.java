//#if MC < 1.9
package com.arcticlauncher.legacy;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;
import net.minecraft.network.packet.s2c.play.ChunkDataS2CPacket;
import net.minecraft.util.math.BlockPos;

/**
 * Opt-in real-play chunk-border regression: -Pdevelopment_scenes=true -Darctic.selftest=ctmborders.
 * A stained-glass ceiling and wall over many chunk and section borders, one ceiling edge exactly on a chunk
 * border. Chunks arrive one at a time (a slow server), the player teleports away and back, flies away and
 * back, and relogs. Two things are measured from the masks the mesher really used: frames drawn while a
 * neighbour chunk is still missing (none are wanted), and the picture once everything has arrived (it must be
 * exactly what the loaded world gives, frame on the real edge included).
 */
final class GlassBordersTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-ctm-borders"); t.setDaemon(true); return t;
	});
	/** The ceiling ends at x = 47, the last block of chunk 2: beyond it is air in another chunk. */
	private static final int R = 40, EDGE = 47, CEILING = 232, WALL_Z = -R - 1, WALL_LOW = 212;
	/** Stained glass is translucent: its sections are also re-sorted while the camera moves. */
	private static final String CEILING_BLOCK = System.getProperty("arctic.ctmborders.ceiling", "minecraft:stained_glass 3");
	/** Last masks the mesher used, down << 8 | south, per block; and the ones the fully loaded world gives. */
	private static final ConcurrentHashMap<Long, Integer> seen = new ConcurrentHashMap<>();
	private static final Map<Long, Integer> settled = new HashMap<>();
	private static volatile boolean watch;
	private static int wrongSettled, framedArriving, framedBesideMissing;
	private static String worldName;
	private GlassBordersTest() {}

	static void start() { later(10, () -> LegacyWorldTest.createTestWorld(() -> { command("tp @p 0.5 222 0.5 0 -90"); later(8, GlassBordersTest::build); })); }
	private static void later(double seconds, Runnable r) { TIMER.schedule(() -> LegacyHooks.onNextFrame(r), (long) (seconds * 1000), TimeUnit.MILLISECONDS); }
	private static void command(String c) { LegacyWorldTest.command(c); }
	private static boolean ceiling(int x, int y, int z) { return y == CEILING && x >= -R && x <= EDGE && Math.abs(z) <= R; }
	private static boolean wall(int x, int y, int z) { return z == WALL_Z && x >= -R && x <= EDGE && y >= WALL_LOW && y < CEILING; }
	static boolean watching(BlockPos p) { return watch && (ceiling(p.getX(), p.getY(), p.getZ()) || wall(p.getX(), p.getY(), p.getZ())); }
	static void observe(BlockPos p, int masks) { seen.put(p.asLong(), masks); }

	private static void build() {
		MinecraftClient mc = MinecraftClient.getInstance();
		worldName = mc.getServer().getLevelName();
		command("gamemode 1");
		for (int x = -R; x <= EDGE; x += 16) {
			int to = Math.min(EDGE, x + 15);
			command("fill " + x + " " + WALL_LOW + " " + WALL_Z + " " + to + " " + (CEILING + 6) + " " + R + " minecraft:air");
			command("fill " + x + " " + CEILING + " " + -R + " " + to + " " + CEILING + " " + R + " " + CEILING_BLOCK);
			command("fill " + x + " " + WALL_LOW + " " + WALL_Z + " " + to + " " + (CEILING - 1) + " " + WALL_Z + " minecraft:stained_glass 11");
		}
		command("fill " + (EDGE + 1) + " " + WALL_LOW + " " + WALL_Z + " " + (EDGE + 8) + " " + (CEILING + 6) + " " + R + " minecraft:air");
		command("fill -1 221 -1 1 221 1 minecraft:stone");
		command("time set 6000"); command("weather clear"); command("gamerule doDaylightCycle false");
		command("tp @p 0.5 222 0.5 0 -90");
		com.arcticlauncher.client.ArcticClient.config().connectedGlass = true;
		mc.options.hudHidden = true;
		later(10, () -> {
			// The reference: every chunk is here, so the world itself says which faces join.
			for (int x = -R; x <= EDGE; x++) {
				for (int z = -R; z <= R; z++) reference(new BlockPos(x, CEILING, z));
				for (int y = WALL_LOW; y < CEILING; y++) reference(new BlockPos(x, y, WALL_Z));
			}
			int edge = settled.get(new BlockPos(EDGE, CEILING, 0).asLong()) >> 8, inner = settled.get(new BlockPos(0, CEILING, 0).asLong()) >> 8;
			ArcticLegacy.LOG.info("ctmborders reference: {} blocks, inner mask {}, mask on the chunk-border edge {}", settled.size(), inner, edge);
			if (inner != 255 || edge == 255) throw new AssertionError("ctmborders: the reference is wrong");
			watch = true;
			mc.worldRenderer.reload();
			overview(() -> { verify("initial"); slow(); });
		});
	}
	private static void reference(BlockPos p) {
		MinecraftClient mc = MinecraftClient.getInstance();
		settled.put(p.asLong(), LegacyGlassCtm.masks(mc.world, mc.world.getBlockState(p), p));
	}

	/** From far below the whole ceiling and wall are in view, so every section that needs a rebuild gets one. */
	private static void overview(Runnable then) { hold(0.5, 150, 0.5, System.nanoTime(), 8, then); }
	private static void hold(double x, double y, double z, long start, double seconds, Runnable then) {
		MinecraftClient mc = MinecraftClient.getInstance();
		if (mc.player == null) return;
		mc.player.abilities.flying = true;
		mc.player.updatePosition(x, y, z);
		mc.player.velocityX = mc.player.velocityY = mc.player.velocityZ = 0;
		mc.player.pitch = -90;
		if ((System.nanoTime() - start) / 1e9 >= seconds) { then.run(); return; }
		later(0.02, () -> hold(x, y, z, start, seconds, then));
	}

	/** Frames the mesher drew that the settled picture does not have, and joins it does not have. */
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
			if (log && framed + joined <= 10) { BlockPos p = BlockPos.fromLong(e.getKey()); first.append(' ').append(p.getX()).append(',').append(p.getY()).append(',').append(p.getZ()).append('=').append(got >> 8).append('/').append(got & 255).append(" want ").append(want >> 8).append('/').append(want & 255); }
		}
		if (log) ArcticLegacy.LOG.info("ctmborders {} {}: meshed={} of {} framed={} wrongly joined={}{}", framed + joined == 0 && meshed > 0 ? "ok" : "WRONG", phase, meshed, settled.size(), framed, joined, first);
		return new int[]{framed, joined, meshed};
	}
	/** Everything has arrived and is in view: the mesh must be the settled picture exactly. */
	private static void verify(String phase) {
		int[] d = differences(true, phase);
		wrongSettled += d[0] + d[1] + (d[2] == settled.size() ? 0 : 1);
		LegacyWorldTest.shot(phase + "-settled");
	}

	/** Samples an arrival: how many frames show that do not belong, the worst moment and how long they stay. */
	private static void sample(String phase, long start, StringBuilder series, int[] peak, boolean[] shot) {
		double t = (System.nanoTime() - start) / 1e9;
		int framed = differences(false, phase)[0];
		if (framed != peak[2]) { peak[2] = framed; series.append(String.format(Locale.ROOT, " %.1fs:%d", t, framed)); }
		if (framed > 0) peak[1] = (int) (t * 1000);
		if (framed > peak[0]) peak[0] = framed;
		if (framed >= 24 && !shot[0]) { shot[0] = true; LegacyWorldTest.shot(phase + "-arriving"); }
		if (t < 12) { later(0.1, () -> sample(phase, start, series, peak, shot)); return; }
		framedArriving += peak[0];
		ArcticLegacy.LOG.info("ctmborders arrival {}: peak framed={} last at {}ms; series{}", phase, peak[0], peak[1], series);
	}
	private static void sample(String phase) { sample(phase, System.nanoTime(), new StringBuilder(), new int[3], new boolean[1]); }

	/**
	 * A server that sends chunks one at a time, nearest first, the way a busy multiplayer server does after a
	 * teleport: every chunk of the build is dropped on the client and delivered again as real chunk packets
	 * while the player keeps moving. The outer columns come much later, like the edge of a view distance.
	 */
	private static void slow() {
		MinecraftClient mc = MinecraftClient.getInstance();
		List<ChunkDataS2CPacket> near = new ArrayList<>(), edge = new ArrayList<>();
		List<int[]> order = new ArrayList<>();
		for (int x = -4; x <= 4; x++) for (int z = -4; z <= 3; z++) order.add(new int[]{x, z});
		order.sort(java.util.Comparator.comparingInt(c -> (2 * c[0] + 1) * (2 * c[0] + 1) + (2 * c[1] + 1) * (2 * c[1] + 1)));
		for (int[] c : order) (c[0] >= 1 || c[0] < -2 ? edge : near).add(new ChunkDataS2CPacket(mc.world.getChunk(c[0], c[1]), true, 65535));
		command("tp @p 0.5 222 0.5 0 -90");
		later(2, () -> {
			seen.clear();
			for (int[] c : order) mc.world.handleChunk(c[0], c[1], false);
			ArcticLegacy.LOG.info("ctmborders slow: dropped {} chunks; {} arrive one per 120 ms, {} later", order.size(), near.size(), edge.size());
			sample("slow");
			wander(0.5, 0.5, System.nanoTime(), near.size() * 0.12 + 4, () -> { });
			deliver(near, 0, () -> later(10, () -> {
				// Chunks 1.. and ..-3 are still missing: x = 15 and x = -32 are the last loaded columns.
				besideMissing("slow-edge");
				hold(16.0, 222, 0.5, System.nanoTime(), 3, () -> {
					LegacyWorldTest.shot("slow-edge");
					deliver(edge, 0, () -> overview(() -> { verify("slow"); teleport(0); }));
				});
			}));
		});
	}
	private static void deliver(List<ChunkDataS2CPacket> packets, int index, Runnable then) {
		if (index == packets.size()) { then.run(); return; }
		MinecraftClient.getInstance().getNetworkHandler().onChunkData(packets.get(index));
		later(0.12, () -> deliver(packets, index + 1, then));
	}
	/** The loaded columns next to chunks that have not arrived. */
	private static void besideMissing(String phase) {
		int framed = 0, meshed = 0;
		for (int z = -R + 1; z < R; z++) for (int x : new int[]{15, -32}) {
			Integer m = seen.get(new BlockPos(x, CEILING, z).asLong());
			if (m == null) continue;
			meshed++;
			if (m >> 8 != 255) framed++;
		}
		int inside = 0;
		for (int z = -R + 1; z < R; z++) for (int x = -31; x < 15; x++) { Integer m = seen.get(new BlockPos(x, CEILING, z).asLong()); if (m != null && m >> 8 != 255) inside++; }
		framedBesideMissing += framed + inside;
		ArcticLegacy.LOG.info("ctmborders {} {}: beside a missing chunk meshed={} framed={}; framed inside the loaded area={}", framed + inside == 0 && meshed > 0 ? "ok" : "FRAMED", phase, meshed, framed, inside);
	}

	/** Far away until everything unloads, then straight back in. Arrival spots differ so chunks arrive in different orders. */
	private static final double[][] ARRIVALS = {{0.5, 0.5}, {15.5, 15.5}, {-16.5, 8.5}, {24.5, -24.5}};
	private static void teleport(int round) {
		if (round == ARRIVALS.length) { fly(); return; }
		command("tp @p 3000.5 222 3000.5 0 -90");
		later(6, () -> {
			seen.clear();
			command("tp @p " + ARRIVALS[round][0] + " 222 " + ARRIVALS[round][1] + " 0 -90");
			sample("teleport" + round);
			wander(ARRIVALS[round][0], ARRIVALS[round][1], System.nanoTime(), 9, () -> later(4, () -> overview(() -> { verify("teleport" + round); teleport(round + 1); })));
		});
	}

	/** Keep moving under the ceiling the way a player does on arrival; every block moved re-sorts translucent sections. */
	private static void wander(double cx, double cz, long start, double seconds, Runnable then) {
		MinecraftClient mc = MinecraftClient.getInstance();
		double t = (System.nanoTime() - start) / 1e9;
		if (mc.player == null) return;
		if (t >= seconds) { mc.player.updatePosition(cx, 222, cz); then.run(); return; }
		mc.player.abilities.flying = true;
		mc.player.updatePosition(cx + Math.sin(t * 2.5) * 6, 222, cz + Math.cos(t * 1.7) * 6);
		mc.player.velocityX = mc.player.velocityY = mc.player.velocityZ = 0;
		mc.player.pitch = -90;
		later(0.02, () -> wander(cx, cz, start, seconds, then));
	}

	/** Creative flight out past the render distance and back, the way a player returns to a build. */
	private static void fly() {
		MinecraftClient mc = MinecraftClient.getInstance();
		command("tp @p 0.5 222 0.5 0 -90");
		later(3, () -> {
			mc.player.abilities.flying = true;
			mc.player.sendAbilitiesUpdate();
			flyStep(System.nanoTime(), (mc.options.viewDistance + 4) * 16 + R * 2, 30);
		});
	}
	private static boolean flyBack;
	private static void flyStep(long start, double far, double speed) {
		MinecraftClient mc = MinecraftClient.getInstance();
		double t = (System.nanoTime() - start) / 1e9 * speed;
		double x = t < far ? t : Math.max(0, 2 * far - t);
		if (t >= far && !flyBack) { flyBack = true; seen.clear(); sample("fly"); }
		mc.player.abilities.flying = true;
		mc.player.updatePosition(x + 0.5, 222, 0.5);
		mc.player.velocityX = mc.player.velocityY = mc.player.velocityZ = 0;
		mc.player.pitch = -90;
		if (t < 2 * far) { later(0.02, () -> flyStep(start, far, speed)); return; }
		later(6, () -> overview(() -> { verify("fly"); relog(); }));
	}

	private static void relog() {
		MinecraftClient mc = MinecraftClient.getInstance();
		command("tp @p 0.5 222 0.5 0 -90");
		later(2, () -> {
			LegacyPlatform.leave(mc);
			seen.clear();
			mc.startIntegratedServer(worldName, worldName, null);
			later(2, () -> awaitWorld(0));
		});
	}
	private static void awaitWorld(int attempt) {
		MinecraftClient mc = MinecraftClient.getInstance();
		mc.options.pauseOnLostFocus = false;
		if (mc.world == null || mc.player == null || mc.currentScreen != null) {
			if (mc.currentScreen instanceof net.minecraft.client.gui.screen.GameMenuScreen) mc.setScreen(null);
			if (attempt > 120) { wrongSettled++; ArcticLegacy.LOG.error("ctmborders FAIL: relog never loaded the world"); finish(); return; }
			later(1, () -> awaitWorld(attempt + 1));
			return;
		}
		mc.options.hudHidden = true;
		sample("relog");
		wander(mc.player.x, mc.player.z, System.nanoTime(), 9, () -> later(4, () -> overview(() -> { verify("relog"); finish(); })));
	}

	private static void finish() {
		boolean pass = wrongSettled == 0 && framedArriving == 0 && framedBesideMissing == 0;
		ArcticLegacy.LOG.info("ctmborders {}: wrong once settled={}, framed beside a missing chunk={}, framed at the arrival peaks={}", pass ? "PASS" : "FAIL", wrongSettled, framedBesideMissing, framedArriving);
		later(2, () -> MinecraftClient.getInstance().scheduleStop());
	}
}
//#endif
