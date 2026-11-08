package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;

/**
 * Development check, only with {@code -Darctic.selftest=badge}: every player counts as an Arctic player,
 * and the player list (a single player gets one with a list objective) and your own name tag (third person,
 * seen from the front) are screenshot for each badge style, each GUI scale and Fancy on and off.
 */
final class LegacyBadgeTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-badgetest");
		t.setDaemon(true);
		return t;
	});
	private static final int STEP = 3;

	private LegacyBadgeTest() {}

	static boolean requested() {
		return "badge".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticLegacy.LOG.info("badgetest: scheduled");
		TIMER.schedule(() -> run(() -> {
			// A test window that lost (or never got) focus would pause the game at once.
			MinecraftClient.getInstance().options.pauseOnLostFocus = false;
			LegacyWorldTest.createTestWorld(LegacyBadgeTest::steps);
		}), 10, TimeUnit.SECONDS);
	}

	private static void run(Runnable r) {
		LegacyHooks.onNextFrame(() -> {
			MinecraftClient mc = MinecraftClient.getInstance();
			if (mc.currentScreen instanceof net.minecraft.client.gui.screen.GameMenuScreen) {
				mc.setScreen(null);
			}
			r.run();
		});
	}

	private static void scale(int n) {
		MinecraftClient mc = MinecraftClient.getInstance();
		mc.options.guiScale = n;
		((com.arcticlauncher.legacy.mixin.MinecraftClientResizeInvoker) mc).arctic$resize(mc.width, mc.height);
	}

	private static void steps() {
		final ClientConfig c = ArcticClient.config();
		final MinecraftClient mc = MinecraftClient.getInstance();
		java.util.List<Runnable> steps = new java.util.ArrayList<>();
		steps.add(() -> {
			LegacyWorldTest.command("time set 6000");
			LegacyWorldTest.command("weather clear");
			LegacyWorldTest.command("tp @p ~ 120 ~ 0 20");
			LegacyWorldTest.command("fill ~-4 119 ~-3 ~4 119 ~7 minecraft:stone");
			LegacyWorldTest.command("fill ~-4 120 ~-3 ~4 124 ~7 minecraft:air");
			LegacyWorldTest.command("scoreboard objectives add arctic dummy");
			LegacyWorldTest.command("scoreboard objectives setdisplay list arctic");
			c.badgeOthersOnly = false;
			c.ownNametag = true;
			c.badgeNametags = true;
			c.badgeWhere = 0;
			mc.options.perspective = 2;
		});
		// {style, scale, fancy}
		int[][] views = {{0, 2, 0}, {1, 2, 0}, {2, 2, 0}, {3, 2, 0}, {0, 1, 0}, {0, 3, 0}, {0, 4, 0}, {0, 2, 1}, {0, 3, 1}, {0, 4, 1}, {0, 1, 1}};
		for (final int[] v : views) {
			steps.add(() -> {
				c.badgeStyle = v[0];
				c.fancy = v[2] == 1;
				LegacySmoothFont.rebuild();
				scale(v[1]);
				LegacyWorldTest.setTab(true);
			});
			steps.add(() -> LegacyWorldTest.shot("badge-tab-s" + v[0] + "-g" + v[1] + "-f" + v[2]));
			steps.add(() -> LegacyWorldTest.setTab(false));
			steps.add(() -> LegacyWorldTest.shot("badge-tag-s" + v[0] + "-g" + v[1] + "-f" + v[2]));
		}
		steps.add(() -> {
			ArcticLegacy.LOG.info("badgetest: done");
			mc.scheduleStop();
		});
		for (int i = 0; i < steps.size(); i++) {
			final Runnable step = steps.get(i);
			TIMER.schedule(() -> run(step), (long) i * STEP, TimeUnit.SECONDS);
		}
	}
}
