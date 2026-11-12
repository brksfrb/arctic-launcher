package com.arcticlauncher.mod;

//#if MC >= 1.21.5
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Fog;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/**
 * Offline-only matrix scene for Fog ({@code -Darctic.selftest=fogmatrix}): every setting (off, "push fog away",
 * "no fog at all") at the render distance edge, in rain, underwater and in the Nether. Each shot logs
 * {@code fog-matrix:} with how many fog distances the hooks saw/changed since the last shot.
 */
final class FogMatrixTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-fog-matrix");
		t.setDaemon(true);
		return t;
	});
	private static long at;
	private static int seen0;
	private static int changed0;

	private FogMatrixTest() {}

	static void start() {
		TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(FogMatrixTest::scene)), 10, TimeUnit.SECONDS);
	}

	private static void run(Runnable r) {
		Minecraft.getInstance().execute(r);
	}

	private static void command(String s) {
		Compat.sendChat("/" + s);
	}

	private static void step(long ms, Runnable r) {
		at += ms;
		TIMER.schedule(() -> run(r), at, TimeUnit.MILLISECONDS);
	}

	private static void set(boolean fog, boolean off, double scale) {
		ClientConfig c = ArcticClient.config();
		c.fog = fog;
		c.fogOff = off;
		c.fogScale = scale;
	}

	private static void shot(String label) {
		ClientConfig c = ArcticClient.config();
		int seen = Fog.seen;
		int changed = Fog.changed;
		ArcticMod.LOG.info("fog-matrix: {} y={} fog={} off={} scale={} factor={} hooks-seen={} hooks-changed={} last={}->{}", label, Minecraft.getInstance().player.getY(), c.fog, c.fogOff, c.fogScale,
				Fog.factor(), seen - seen0, changed - changed0, Fog.lastIn, Fog.lastOut);
		seen0 = seen;
		changed0 = changed;
		WorldTest.shot(label);
	}

	private static void settings(String place) {
		step(100, () -> set(false, false, 3));
		step(3000, () -> shot(place + "-1-off"));
		step(100, () -> set(true, false, 3));
		step(3000, () -> shot(place + "-2-push"));
		step(100, () -> set(true, true, 3));
		step(3000, () -> shot(place + "-3-nofog"));
		step(100, () -> set(false, false, 3));
		step(2500, () -> shot(place + "-4-off-again"));
	}

	/** A stone floor and magenta pillars on alternating sides of the view line, 24 blocks apart out to 110. */
	private static void pillars() {
		for (int z0 = -12; z0 < 180; z0 += 64) {
			command("fill -10 94 " + z0 + " 10 99 " + (z0 + 63) + " minecraft:stone");
			command("fill -10 100 " + z0 + " 10 112 " + (z0 + 63) + " minecraft:air");
		}
		for (int z = 30, i = 0; z <= 110; z += 16, i++) {
			int x = i % 2 == 0 ? -6 : 3;
			command("fill " + x + " 100 " + z + " " + (x + 3) + " 110 " + z + " minecraft:magenta_wool");
		}
	}

	private static void scene() {
		command("gamemode creative");
		command("gamerule doDaylightCycle false");
		command("gamerule doWeatherCycle false");
		command("time set 6000");
		command("weather clear");
		command("tp @s 0 100 0 0 0");
		at = 0;
		step(9000, () -> {
			pillars();
			//#if MC >= 26.2
			if (!Compat.hudHidden()) {
				Minecraft.getInstance().gui.hud.toggle();
			}
			//#else
			Minecraft.getInstance().options.hideGui = true;
			//#endif
		});
		step(3000, FogMatrixTest::pillars);
		step(2000, () -> command("tp @s 0 100 0 0 0"));
		settings("a-edge");
		step(100, () -> command("weather rain"));
		step(4000, () -> {});
		settings("b-rain");
		step(100, () -> {
			command("weather clear");
			command("fill -3 95 -3 3 99 3 minecraft:water");
			command("tp @s 0 97.2 0 0 0");
		});
		settings("c-water");
		step(100, () -> {
			command("fill -3 95 -3 3 99 3 minecraft:stone");
			command("execute in minecraft:the_nether run tp @s 0 100 0 0 0");
		});
		step(6000, FogMatrixTest::pillars);
		step(2000, FogMatrixTest::pillars);
		step(2000, () -> command("tp @s 0 100 0 0 0"));
		settings("d-nether");
		step(500, () -> {
			ArcticMod.LOG.info("fog-matrix: done");
			Minecraft.getInstance().stop();
		});
	}
}
//#endif
