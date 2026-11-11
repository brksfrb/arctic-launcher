package com.arcticlauncher.legacy;
import com.arcticlauncher.client.ArcticClient;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
/** Offline-only, opt-in visual regression scene: -Darctic.selftest=ctm. */
final class GlassCtmTest {
	private static final java.util.concurrent.ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-ctm-proof"); t.setDaemon(true); return t;
	});
	private GlassCtmTest() {}
	static void start() {
		TIMER.schedule(() -> run(() -> LegacyWorldTest.createTestWorld(GlassCtmTest::scene)), 10, TimeUnit.SECONDS);
	}
	private static void run(Runnable r) { LegacyHooks.onNextFrame(r); }
	private static void command(String s) { LegacyWorldTest.command(s); }
	private static void scene() {
		command("tp @p 0.5 220 -7 0 0");
		TIMER.schedule(() -> run(GlassCtmTest::buildScene), 7, TimeUnit.SECONDS);
	}
	private static void buildScene() {
		ArcticLegacy.LOG.info("ctm-proof: native available={}, enabled={}", LegacyGlassCtm.available(), LegacyGlassCtm.enabled());
		command("gamemode creative");
		command("fill -10 219 -10 10 219 8 minecraft:stone");
		command("fill -10 220 -10 10 230 8 minecraft:air");
		command("fill -7 220 4 7 226 4 minecraft:stone");
		command("fill -5 220 0 -3 222 0 minecraft:glass");
		command("fill -1 220 0 1 222 0 minecraft:stained_glass 14");
		command("fill 3 221 0 5 221 0 minecraft:glass_pane");
		// Floor zone, seen from a pit below: the inner faces of adjoining glass blocks must not show.
		command("fill -10 215 -20 10 215 -11 minecraft:stone");
		command("fill -10 216 -20 10 230 -11 minecraft:air");
		command("fill -7 224 -16 -5 224 -14 minecraft:stained_glass 5");
		command("setblock -4 224 -15 minecraft:stained_glass 13");
		command("fill -2 224 -16 0 225 -14 minecraft:stained_glass 5");
		command("fill 2 224 -16 4 224 -14 minecraft:glass");
		command("fill 5 224 -15 8 224 -15 minecraft:glass_pane");
		command("time set 6000"); command("weather clear"); command("tp @p 0.5 220 -7 0 0");
		MinecraftClient.getInstance().options.hudHidden = true;
		ArcticClient.config().connectedGlass = true;
		TIMER.schedule(() -> run(() -> { check(true); LegacyWorldTest.shot("ctm-on"); }), 8, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> command("tp @p 0.5 216 -15 0 -90")), 9, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> LegacyWorldTest.shot("ctm-floor-below-on")), 14, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> command("tp @p 0.5 216 -19.5 0 -55")), 15, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> LegacyWorldTest.shot("ctm-floor-oblique-on")), 20, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> { ArcticClient.config().connectedGlass = false; command("tp @p 0.5 220 -7 0 0"); }), 21, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> { check(false); LegacyWorldTest.shot("ctm-off"); }), 30, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> command("tp @p 0.5 216 -15 0 -90")), 31, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> LegacyWorldTest.shot("ctm-floor-below-off")), 36, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> command("tp @p 0.5 216 -19.5 0 -55")), 37, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> LegacyWorldTest.shot("ctm-floor-oblique-off")), 42, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> MinecraftClient.getInstance().scheduleStop()), 46, TimeUnit.SECONDS);
	}
	private static int passed, failed;
	private static void expect(boolean hidden, int x, int y, int z, Direction face, boolean edge, boolean boundary, String what) {
		boolean actual = LegacyGlassCtm.hides(MinecraftClient.getInstance().world, new BlockPos(x, y, z), face, edge, boundary);
		if (actual == hidden) { passed++; return; }
		failed++;
		ArcticLegacy.LOG.error("ctm-proof: FAIL {} at {},{},{} {}: expected {}, was {}", what, x, y, z, face, hidden ? "hidden" : "kept", actual ? "hidden" : "kept");
	}
	/** The scene's blocks through the real neighbour test: what a flush quad of each kind does. */
	private static void check(boolean enabled) {
		passed = failed = 0;
		Direction[] sides = {Direction.NORTH, Direction.SOUTH, Direction.WEST, Direction.EAST};
		// 3x3 lime floor: inner faces go, the underside and the rim stay.
		for (Direction side : sides) expect(enabled, -6, 224, -15, side, false, true, "lime floor centre side");
		expect(false, -6, 224, -15, Direction.DOWN, false, true, "lime floor underside");
		expect(false, -6, 224, -15, Direction.UP, false, true, "lime floor top");
		for (int x = -7; x <= -5; x++) for (int z = -16; z <= -14; z++) expect(false, x, 224, z, Direction.DOWN, false, true, "lime floor underside");
		expect(false, -7, 224, -15, Direction.WEST, false, true, "lime floor rim");
		expect(enabled, -7, 224, -15, Direction.EAST, false, true, "lime floor rim inner face");
		expect(false, -7, 224, -16, Direction.WEST, false, true, "lime floor corner rim");
		expect(false, -7, 224, -16, Direction.NORTH, false, true, "lime floor corner rim");
		expect(enabled, -7, 224, -16, Direction.EAST, false, true, "lime floor corner inner face");
		expect(enabled, -7, 224, -16, Direction.SOUTH, false, true, "lime floor corner inner face");
		expect(false, -6, 224, -15, Direction.NORTH, false, false, "quad inside the cube");
		// Another colour is another glass.
		expect(false, -5, 224, -15, Direction.EAST, false, true, "lime beside green");
		expect(false, -4, 224, -15, Direction.WEST, false, true, "green beside lime");
		// Two layers: the faces between them go, the outside stays.
		expect(enabled, -1, 224, -15, Direction.UP, false, true, "lower layer top");
		expect(enabled, -1, 225, -15, Direction.DOWN, false, true, "upper layer underside");
		expect(false, -1, 224, -15, Direction.DOWN, false, true, "lower layer underside");
		expect(false, -1, 225, -15, Direction.UP, false, true, "upper layer top");
		// Clear glass behaves the same.
		for (Direction side : sides) expect(enabled, 3, 224, -15, side, false, true, "clear floor centre side");
		expect(false, 3, 224, -15, Direction.DOWN, false, true, "clear floor underside");
		expect(false, 2, 224, -15, Direction.WEST, false, true, "clear floor rim");
		// A pane beside a block connects to neither; panes still drop the edge strips between them only.
		expect(false, 4, 224, -15, Direction.EAST, false, true, "clear block beside pane");
		expect(false, 5, 224, -15, Direction.WEST, true, true, "pane edge beside block");
		expect(enabled, 5, 224, -15, Direction.EAST, true, true, "pane edge beside pane");
		expect(enabled, 6, 224, -15, Direction.WEST, true, true, "pane edge beside pane");
		expect(enabled, 6, 224, -15, Direction.EAST, true, true, "pane edge beside pane");
		expect(false, 8, 224, -15, Direction.EAST, true, true, "pane end edge");
		expect(false, 6, 224, -15, Direction.EAST, false, true, "pane glass face");
		expect(false, 6, 224, -15, Direction.EAST, true, false, "pane edge inside the cube");
		ArcticLegacy.LOG.info("ctm-proof: assertions ctm={} passed={} failed={}", enabled, passed, failed);
	}
}
