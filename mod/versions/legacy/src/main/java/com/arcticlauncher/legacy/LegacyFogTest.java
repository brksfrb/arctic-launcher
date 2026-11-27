package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;
import org.lwjgl.opengl.GL11;

/**
 * Offline-only visual scene for Fog ({@code -Darctic.selftest=fog}): a stone runway with pillars, then Fog off,
 * stretched, "No fog at all", rapid toggling, underwater and blind, a screenshot in each state. It also logs the
 * GL fog start/end it sees, so a fog that compounds (or turns to Inf/NaN) shows up in the numbers.
 */
final class LegacyFogTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-fog-proof");
		t.setDaemon(true);
		return t;
	});
	private static long at;

	private LegacyFogTest() {}

	static boolean requested() {
		return "fog".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		TIMER.schedule(() -> game(() -> LegacyWorldTest.createTestWorld(LegacyFogTest::scene, 4242L)), 10, TimeUnit.SECONDS);
	}

	private static void game(Runnable r) {
		LegacyHooks.onNextFrame(r);
	}

	private static void command(String s) {
		LegacyWorldTest.command(s);
	}

	/** Runs {@code r} on the game thread {@code ms} after the previous step. */
	private static void step(long ms, Runnable r) {
		at += ms;
		TIMER.schedule(() -> game(r), at, TimeUnit.MILLISECONDS);
	}

	/** -Darctic.fogtest.control=true: never turn Fog on (a control run: what the world looks like without it). */
	private static final boolean CONTROL = Boolean.getBoolean("arctic.fogtest.control");

	private static void set(boolean fog, boolean off, double scale) {
		ClientConfig c = ArcticClient.config();
		if (CONTROL) {
			fog = false;
		}
		c.fog = fog;
		c.fogOff = off;
		c.fogScale = scale;
	}

	private static void shot(String label) {
		ClientConfig c = ArcticClient.config();
		ArcticLegacy.LOG.info("fog-proof: {} fog={} off={} scale={} glStart={} glEnd={}", label, c.fog, c.fogOff, c.fogScale,
				GL11.glGetFloat(GL11.GL_FOG_START), GL11.glGetFloat(GL11.GL_FOG_END));
		MinecraftClient mc = MinecraftClient.getInstance();
		ArcticLegacy.LOG.info("fog-proof: {} at {} {} {} yaw={} pitch={} below={}", label, mc.player.x, mc.player.y, mc.player.z, mc.player.yaw, mc.player.pitch,
				mc.world.getBlockState(new net.minecraft.util.math.BlockPos(0, 219, 0)).getBlock());
		LegacyWorldTest.shot(label);
	}

	private static void scene() {
		command("gamemode creative");
		command("gamerule doDaylightCycle false");
		command("time set 6000");
		command("weather clear");
		command("tp @p 0 220 0 0 0");
		at = 0;
		// Build once the chunks around are there (a fill into an unloaded chunk does nothing).
		step(6000, () -> {
			command("fill -12 214 -12 12 219 100 minecraft:stone");
			command("fill -12 220 -12 12 232 100 minecraft:air");
			for (int z = 20; z <= 100; z += 20) {
				command("fill -3 220 " + z + " 3 228 " + z + " minecraft:wool " + (z / 20 * 2 % 16));
			}
		});
		MinecraftClient mc = MinecraftClient.getInstance();
		mc.options.hudHidden = true;
		mc.focused = true;
		step(3000, () -> {
			command("tp @p 0 220 0 0 0");
			set(false, false, 3);
		});
		step(3000, () -> shot("01-off"));
		step(100, () -> set(true, false, 3));
		step(3000, () -> shot("02-scale3"));
		step(100, () -> set(true, false, 10));
		step(3000, () -> shot("03-scale10"));
		step(100, () -> set(true, true, 3));
		step(3000, () -> shot("04-nofog"));
		step(100, () -> set(false, true, 3));
		step(2000, () -> shot("05-off-after-nofog"));
		// Rapid toggling, ending off.
		for (int i = 0; i < 12; i++) {
			final boolean on = i % 2 == 0;
			step(250, () -> set(on, true, 3));
		}
		step(100, () -> set(false, true, 3));
		step(2000, () -> shot("06-off-after-toggles"));
		step(100, () -> set(true, true, 3));
		step(2000, () -> shot("07-nofog-again"));
		step(100, () -> set(false, true, 3));
		step(1500, () -> shot("08-off-again"));
		// Underwater.
		step(100, () -> {
			command("fill -3 215 -3 3 219 3 minecraft:water");
			command("tp @p 0 217.2 0 0 0");
			set(true, true, 3);
		});
		step(3000, () -> shot("09-underwater-nofog"));
		step(100, () -> {
			command("fill -3 215 -3 3 219 3 minecraft:stone");
			command("tp @p 0 220 -6 0 0");
		});
		step(3000, () -> shot("10-after-water-nofog"));
		step(100, () -> set(false, true, 3));
		step(2000, () -> shot("11-after-water-off"));
		// Blindness.
		step(100, () -> {
			command("effect @p 15 5 1");
			set(true, true, 3);
		});
		step(2500, () -> shot("12-blind-nofog"));
		step(4500, () -> shot("13-after-blind-nofog"));
		step(100, () -> set(false, true, 3));
		step(2000, () -> shot("14-after-blind-off"));
		step(500, () -> {
			ArcticLegacy.LOG.info("fog-proof: done");
			mc.scheduleStop();
		});
	}
}
