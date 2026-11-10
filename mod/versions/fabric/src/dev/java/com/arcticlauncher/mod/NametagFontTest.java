package com.arcticlauncher.mod;

//#if MC >= 1.21.11
import com.arcticlauncher.client.ArcticClient;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/** Opt-in real-world font regression capture, with the old atlas and buckets at the identical camera. */
final class NametagFontTest {
	private static int baseWidth;
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-nametagfont-test");
		t.setDaemon(true);
		return t;
	});

	private NametagFontTest() {}

	static void start() {
		TIMER.schedule(() -> run(() -> {
			ArcticClient.config().fancy = true;
			ArcticClient.config().fontAntialias = true;
			ArcticClient.saveConfig();
			//#if MC >= 26.1
			ArcticPacks.setSmoothFont(true);
			//#else
			SmoothFont.changed();
			//#endif
			WorldTest.createTestWorld(NametagFontTest::scene);
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
		later(4, () -> {
			command("fill -5 199 -5 5 199 5 stone");
			command("tp @s 0.5 200 0.5 0 0");
			command("summon armor_stand 0.5 199.5 1.35 {CustomName:\"102ms | 3MAR_GAMER_YT\",CustomNameVisible:1b,NoGravity:1b,Invisible:1b}");
			command("title @a times 0 1200 0");
			command("title @a subtitle {\"text\":\"Smooth title reference\"}");
			command("title @a title {\"text\":\"Starting in\"}");
		});
		later(5, () -> {
			// Reset client interpolation and hide chat so both frames have the same view.
			mc.player.setYRot(0); mc.player.setXRot(0);
			Compat.setScreen(null);
			TextDrawScale.proofBaseline = true;
		});
		later(10, () -> {
			baseWidth = mc.font.width("102ms | 3MAR_GAMER_YT");
			WorldTest.shot("nametagfont-before");
			ArcticMod.LOG.info("nametagfont: BEFORE old GUI-size atlas; camera 0.5 200 0.5 yaw 0 pitch 0; entity 0.5 199.5 1.35");
		});
		later(12, () -> TextDrawScale.proofBaseline = false);
		later(17, () -> {
			int width = mc.font.width("102ms | 3MAR_GAMER_YT");
			if (width != baseWidth) throw new AssertionError("font width changed " + baseWidth + " -> " + width);
			ArcticMod.LOG.info("nametagfont: width unchanged: {}", width);
			WorldTest.shot("nametagfont-after");
			ArcticMod.LOG.info("nametagfont: AFTER projected size buckets; identical camera; gui scale 2");
		});
		later(21, () -> {
			ArcticClient.config().fancy = false;
			ArcticClient.saveConfig();
			//#if MC >= 26.1
			ArcticPacks.setSmoothFont(false);
			//#else
			SmoothFont.changed();
			//#endif
		});
		later(28, () -> {
			WorldTest.shot("nametagfont-off");
			ArcticMod.LOG.info("nametagfont: smooth font OFF, width {}", mc.font.width("102ms | 3MAR_GAMER_YT"));
		});
		later(32, () -> {
			ArcticMod.LOG.info("nametagfont: DONE");
			mc.stop();
		});
	}
}
//#endif
