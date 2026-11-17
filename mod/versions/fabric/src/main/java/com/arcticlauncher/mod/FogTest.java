package com.arcticlauncher.mod;

//#if MC >= 1.21.6
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Fog;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/** Offline-only visual scene for Fog ({@code -Darctic.selftest=fog}); the same steps as the 1.8.9 one. */
final class FogTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-fog-proof");
		t.setDaemon(true);
		return t;
	});
	private static final boolean CONTROL = Boolean.getBoolean("arctic.fogtest.control");
	private static long at;

	private FogTest() {}

	static void start() {
		TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(FogTest::scene)), 10, TimeUnit.SECONDS);
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
		c.fog = CONTROL ? false : fog;
		c.fogOff = off;
		c.fogScale = scale;
	}

	private static void shot(String label) {
		ClientConfig c = ArcticClient.config();
		ArcticMod.LOG.info("fog-proof: {} fog={} off={} scale={} factor={} stretched(96)={}", label, c.fog, c.fogOff, c.fogScale, Fog.factor(), Fog.stretch(96f));
		WorldTest.shot(label);
	}

	private static void scene() {
		command("gamemode creative");
		command("gamerule doDaylightCycle false");
		command("time set 6000");
		command("weather clear");
		command("tp @s 0 220 0 0 0");
		at = 0;
		step(6000, () -> {
			command("fill -12 214 -12 12 219 100 minecraft:stone");
			command("fill -12 220 -12 12 232 100 minecraft:air");
			for (int z = 20; z <= 100; z += 20) {
				command("fill -3 220 " + z + " 3 228 " + z + " minecraft:magenta_wool");
			}
			//#if MC >= 26.2
			if (!Compat.hudHidden()) {
				Minecraft.getInstance().gui.hud.toggle();
			}
			//#else
			Minecraft.getInstance().options.hideGui = true;
			//#endif
		});
		step(3000, () -> {
			command("tp @s 0 220 0 0 0");
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
		step(100, () -> {
			command("fill -3 215 -3 3 219 3 minecraft:water");
			command("tp @s 0 217.2 0 0 0");
			set(true, true, 3);
		});
		step(3000, () -> shot("09-underwater-nofog"));
		step(100, () -> {
			command("fill -3 215 -3 3 219 3 minecraft:stone");
			command("tp @s 0 220 -6 0 0");
		});
		step(3000, () -> shot("10-after-water-nofog"));
		step(100, () -> set(false, true, 3));
		step(2000, () -> shot("11-after-water-off"));
		step(100, () -> {
			command("effect give @s minecraft:blindness 5 1 true");
			set(true, true, 3);
		});
		step(2500, () -> shot("12-blind-nofog"));
		step(4500, () -> shot("13-after-blind-nofog"));
		step(100, () -> set(false, true, 3));
		step(2000, () -> shot("14-after-blind-off"));
		step(500, () -> {
			ArcticMod.LOG.info("fog-proof: done");
			Minecraft.getInstance().stop();
		});
	}
}
//#endif
