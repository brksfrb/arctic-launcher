package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.menu.Menus;
import java.io.File;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;

/** -Darctic.selftest=friendsvc: the Friends page with voice chat off and on, the status line, and the voice settings page. */
final class LegacyFriendsVcTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-friendsvc");
		t.setDaemon(true);
		return t;
	});

	private LegacyFriendsVcTest() {}

	static void start() {
		later(8, () -> LegacyWorldTest.createTestWorld(() -> {
			later(6, () -> {
				Menus.open("friends");
				status("Screenshot sent.");
			});
			later(12, () -> shot("friendsvc-off"));
			later(13, () -> ArcticClient.voice().setEnabled(true));
			later(14, () -> Menus.open("friends"));
			later(18, () -> shot("friendsvc-on"));
			later(19, () -> Menus.open("voice"));
			later(23, () -> shot("friendsvc-voice-page"));
			later(24, () -> ArcticClient.voice().setEnabled(false));
			later(28, () -> MinecraftClient.getInstance().scheduleStop());
		}));
	}

	private static void status(String text) {
		try {
			java.lang.reflect.Field f = com.arcticlauncher.client.social.Social.class.getDeclaredField("status");
			f.setAccessible(true);
			f.set(ArcticClient.social(), text);
		} catch (ReflectiveOperationException e) {
			ArcticLegacy.LOG.warn("friendsvc: no status", e);
		}
	}

	private static void shot(String name) {
		MinecraftClient mc = MinecraftClient.getInstance();
		net.minecraft.client.util.ScreenshotUtils.saveScreenshot(mc.runDirectory, mc.width, mc.height, mc.getFramebuffer());
		File dir = new File(mc.runDirectory, "screenshots");
		File newest = null;
		File[] files = dir.listFiles();
		if (files != null) {
			for (File f : files) {
				if (f.getName().endsWith(".png") && (newest == null || f.lastModified() > newest.lastModified())) newest = f;
			}
		}
		if (newest != null) newest.renameTo(new File(dir, name + ".png"));
		ArcticLegacy.LOG.info("friendsvc: shot {}", name);
	}

	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> LegacyHooks.onNextFrame(action), seconds, TimeUnit.SECONDS);
	}
}
