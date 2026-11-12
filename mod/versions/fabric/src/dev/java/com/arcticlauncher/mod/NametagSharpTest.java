package com.arcticlauncher.mod;

//#if MC >= 1.21.11
import com.arcticlauncher.client.ArcticClient;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/**
 * Nametag sharpness capture (-Darctic.selftest=nametagsharp): one named stand, the camera steps through
 * distances (and sub-pixel offsets), one screenshot each. -Darctic.nts.smooth=false turns the smooth font off.
 */
final class NametagSharpTest {
	private static final double[] DISTANCES = {18, 11, 7, 4.5, 3, 2, 1.3, 0.9, 0.6, 0.45};
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-nametagsharp-test");
		t.setDaemon(true);
		return t;
	});

	private NametagSharpTest() {}

	static void start() {
		boolean smooth = !"false".equals(System.getProperty("arctic.nts.smooth"));
		TIMER.schedule(() -> run(() -> {
			ArcticClient.config().fancy = smooth;
			ArcticClient.config().fontAntialias = true;
			ArcticClient.saveConfig();
			//#if MC >= 26.1
			ArcticPacks.setSmoothFont(smooth);
			//#else
			SmoothFont.changed();
			//#endif
			WorldTest.createTestWorld(NametagSharpTest::scene);
		}), 10, TimeUnit.SECONDS);
	}

	private static void run(Runnable action) {
		Minecraft.getInstance().execute(action);
	}

	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> run(action), seconds, TimeUnit.SECONDS);
	}

	private static void command(String command) {
		Compat.sendChat("/" + command);
	}

	private static void scene() {
		Minecraft mc = Minecraft.getInstance();
		mc.options.pauseOnLostFocus = false;
		//#if MC >= 26.1
		command("gamerule minecraft:advance_time false");
		//#else
		command("gamerule doDaylightCycle false");
		//#endif
		command("time set 6000");
		command("weather clear");
		command("tp @s 0.5 200 0.5 0 0");
		if (!"false".equals(System.getProperty("arctic.nts.chat"))) {
			// A busy chat first: the GUI-size font atlas holds many neighbours, as in a long session.
			Compat.sendChat("The quick brown fox jumps over the lazy dog 0123456789 | lil Il1 !");
			Compat.sendChat("[Rank] Mod: \"seen\" (top1) #BlocksMC.com <Tokens> 32682 {Gifts} 200 ~ lazys reported j4zqq for AutoClicker");
		}
		later(4, () -> {
			command("fill -5 199 -40 5 199 5 stone");
			command("summon armor_stand 0.5 199.5 5.5 {CustomName:{\"text\":\"seentop1\",\"color\":\"green\"},CustomNameVisible:1b,NoGravity:1b,Invisible:1b}");
			command("summon armor_stand 0.5 198.5 5.5 {CustomName:\"102ms | 3MAR_GAMER_YT\",CustomNameVisible:1b,NoGravity:1b,Invisible:1b}");
			Compat.setScreen(null);
		});
		for (int i = 0; i < DISTANCES.length; i++) {
			final int n = i;
			later(8 + i * 6, () -> {
				double d = DISTANCES[n];
				// Fractional camera x so the quad lands on a different sub-pixel every step.
				command(String.format(java.util.Locale.ROOT, "tp @s %.3f 200 %.3f 0 0", 0.5 + n * 0.0371, 5.5 - d - 0.5 + 0.5));
				Compat.setScreen(null);
			});
			later(11 + i * 6, () -> {
				mc.player.setYRot(0);
				mc.player.setXRot(0);
				WorldTest.shot("nts-d" + DISTANCES[n]);
			});
		}
		later(14 + DISTANCES.length * 6, () -> {
			ArcticMod.LOG.info("nametagsharp: DONE");
			mc.stop();
		});
	}
}
//#endif
