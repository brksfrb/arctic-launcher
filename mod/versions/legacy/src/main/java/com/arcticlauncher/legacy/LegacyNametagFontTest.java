package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;

/** Opt-in real-world font regression capture, with the old atlas and buckets at the identical camera. */
final class LegacyNametagFontTest {
	private static int baseWidth;
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-nametagfont-test");
		t.setDaemon(true);
		return t;
	});

	private LegacyNametagFontTest() {}

	static void start() {
		TIMER.schedule(() -> run(() -> {
			ArcticClient.config().fancy = true;
			ArcticClient.config().fontAntialias = true;
			ArcticClient.saveConfig();

			LegacyWorldTest.createTestWorld(LegacyNametagFontTest::scene);
		}), 10, TimeUnit.SECONDS);
	}

	private static void run(Runnable action) {
		LegacyHooks.onNextFrame(action);
	}

	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> run(action), seconds, TimeUnit.SECONDS);
	}

	private static void command(String command) {
		LegacyWorldTest.command(command);
	}

	private static void scene() {
		MinecraftClient mc = MinecraftClient.getInstance();
		mc.options.pauseOnLostFocus = false;
		command("gamerule doDaylightCycle false");
		command("time set 6000");
		command("weather clear");
		command("tp @p 0.5 200 0.5 0 0");
		later(4, () -> {
			command("fill -5 199 -5 5 199 5 stone");
			command("tp @p 0.5 200 0.5 0 0");
			//#if MC >= 1.11
			command("summon armor_stand 0.5 199.5 1.35 {NoGravity:1b,Invisible:1b}");
			command("entitydata @e[type=armor_stand,r=5] {CustomName:\"102ms | 3MAR_GAMER_YT\",CustomNameVisible:1}");
			//#else
			command("summon ArmorStand 0.5 199.5 1.35 {NoGravity:1b,Invisible:1b}");
			command("entitydata @e[type=ArmorStand,r=5] {CustomName:\"102ms | 3MAR_GAMER_YT\",CustomNameVisible:1}");
			//#endif
			command("title @a times 0 1200 0");
			command("title @a subtitle {\"text\":\"Smooth title reference\"}");
			command("title @a title {\"text\":\"Starting in\"}");
		});
		later(5, () -> {
			// Reset client interpolation and hide chat so both frames have the same view.
			mc.player.yaw = 0; mc.player.pitch = 0;
			mc.setScreen(null);
			LegacySmoothFont.proofBaseline = true;
		});
		later(10, () -> {
			baseWidth = mc.textRenderer.getStringWidth("102ms | 3MAR_GAMER_YT");
			LegacyWorldTest.shot("nametagfont-before");
			ArcticLegacy.LOG.info("nametagfont: BEFORE old GUI-size atlas; camera 0.5 200 0.5 yaw 0 pitch 0; entity 0.5 199.5 1.35");
		});
		later(12, () -> LegacySmoothFont.proofBaseline = false);
		later(17, () -> {
			int width = mc.textRenderer.getStringWidth("102ms | 3MAR_GAMER_YT");
			if (width != baseWidth) throw new AssertionError("font width changed " + baseWidth + " -> " + width);
			ArcticLegacy.LOG.info("nametagfont: width unchanged: {}", width);
			LegacyWorldTest.shot("nametagfont-after");
			ArcticLegacy.LOG.info("nametagfont: AFTER projected size buckets; identical camera; gui scale 2");
		});
		later(21, () -> {
			ArcticClient.config().fancy = false;
			ArcticClient.saveConfig();
		});
		later(28, () -> {
			LegacyWorldTest.shot("nametagfont-off");
			ArcticLegacy.LOG.info("nametagfont: smooth font OFF, width {}", mc.textRenderer.getStringWidth("102ms | 3MAR_GAMER_YT"));
		});
		later(32, () -> {
			ArcticLegacy.LOG.info("nametagfont: DONE");
			mc.scheduleStop();
		});
	}
}
