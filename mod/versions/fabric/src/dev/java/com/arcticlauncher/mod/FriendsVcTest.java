package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.menu.Menus;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/**
 * -Darctic.selftest=friendsvc: the Friends page with voice chat off and on (the switch goes through the
 * launcher bridge; run it against a fake bridge), the status line at the bottom, and the voice settings page.
 */
final class FriendsVcTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-friendsvc");
		t.setDaemon(true);
		return t;
	});

	private FriendsVcTest() {}

	static void start() {
		later(8, () -> WorldTest.createTestWorld(() -> {
			later(6, () -> {
				Menus.open("friends");
				status("Screenshot sent.");
			});
			later(12, () -> WorldTest.shot("friendsvc-off"));
			later(13, () -> ArcticClient.voice().setEnabled(true));
			later(14, () -> Menus.open("friends"));
			later(18, () -> WorldTest.shot("friendsvc-on"));
			later(19, () -> Menus.open("voice"));
			later(23, () -> WorldTest.shot("friendsvc-voice-page"));
			later(24, () -> ArcticClient.voice().setEnabled(false));
			later(28, () -> Minecraft.getInstance().stop());
		}));
	}

	private static void status(String text) {
		try {
			java.lang.reflect.Field f = com.arcticlauncher.client.social.Social.class.getDeclaredField("status");
			f.setAccessible(true);
			f.set(ArcticClient.social(), text);
		} catch (ReflectiveOperationException e) {
			ArcticMod.LOG.warn("friendsvc: no status", e);
		}
	}

	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> Minecraft.getInstance().execute(action), seconds, TimeUnit.SECONDS);
	}
}
