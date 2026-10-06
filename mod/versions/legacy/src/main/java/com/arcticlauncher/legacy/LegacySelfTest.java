package com.arcticlauncher.legacy;

import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.menu.Menus;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.util.ScreenshotUtils;

/**
 * Development check, only with {@code -Darctic.selftest=true}: walks through
 * the Arctic screens taking a screenshot of each, then quits.
 */
final class LegacySelfTest {
	private static final int STEP_SECONDS = 4;
	private static final String[] TABS = {"mods", "hud.fps", "hud.keystrokes", "crosshair", "packs", "looks", "style"};
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-selftest");
		t.setDaemon(true);
		return t;
	});

	private LegacySelfTest() {}

	static void maybeStart() {
		String mode = System.getProperty("arctic.selftest");
		if (mode == null) {
			return;
		}
		if (LegacyReplayTest.requested()) {
			LegacyReplayTest.start();
			return;
		}
		if (LegacyKeysTest.requested()) {
			LegacyKeysTest.start();
			return;
		}
		if (LegacyWorldTest.requested()) {
			LegacyWorldTest.start();
			return;
		}
		if (LegacyVisualsTest.requested()) {
			LegacyVisualsTest.start();
			return;
		}
		ArcticLegacy.LOG.info("selftest: scheduled ({})", mode);
		int step = 3;
		later(step++, () -> shot("title"));
		for (final String tab : TABS) {
			later(step++, () -> {
				Menus.select(tab);
				ArcticClient.platform().openPage(Menus.selected());
			});
			later(step++, () -> shot("menu-" + tab));
		}
		later(step, () -> {
			ArcticLegacy.LOG.info("selftest: done");
			MinecraftClient.getInstance().scheduleStop();
		});
	}

	private static void later(int step, Runnable r) {
		TIMER.schedule(() -> MinecraftClient.getInstance().submit(r), (long) step * STEP_SECONDS, TimeUnit.SECONDS);
	}

	private static void shot(String name) {
		MinecraftClient mc = MinecraftClient.getInstance();
		ScreenshotUtils.saveScreenshot(mc.runDirectory, mc.width, mc.height, mc.getFramebuffer());
		ArcticLegacy.LOG.info("selftest: screenshot {}", name);
	}
}
