package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/**
 * Development check, only with {@code -Darctic.selftest=badge}: every player counts as an Arctic player,
 * and the player list (a single player gets one with a list objective) and your own name tag (third person,
 * seen from the front) are screenshot for each badge style, GUI scale and Fancy on and off.
 */
final class BadgeTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-badgetest");
		t.setDaemon(true);
		return t;
	});
	private static final int STEP = 3;

	private BadgeTest() {}

	static boolean requested() {
		return "badge".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticMod.LOG.info("badgetest: scheduled");
		TIMER.schedule(() -> run(() -> {
			// A test window that lost (or never got) focus would pause the game at once.
			Minecraft.getInstance().options.pauseOnLostFocus = false;
			WorldTest.createTestWorld(BadgeTest::steps);
		}), 10, TimeUnit.SECONDS);
	}

	private static void run(Runnable r) {
		Minecraft.getInstance().execute(() -> {
			if (Minecraft.getInstance().level != null) {
				Compat.setScreen(null);
			}
			r.run();
		});
	}

	private static void command(String command) {
		Compat.sendChat("/" + command);
	}

	private static void camera(boolean front) {
		//#if MC >= 1.16
		Minecraft.getInstance().options.setCameraType(front ? net.minecraft.client.CameraType.THIRD_PERSON_FRONT
				: net.minecraft.client.CameraType.FIRST_PERSON);
		//#else
		Minecraft.getInstance().options.thirdPersonView = front ? 2 : 0;
		//#endif
	}

	private static void scale(int n) {
		Minecraft mc = Minecraft.getInstance();
		//#if MC >= 1.15
		mc.getWindow().setGuiScale(n);
		//#elif MC >= 1.14
		mc.window.setGuiScale(n);
		//#endif
	}

	private static void steps() {
		final ClientConfig c = ArcticClient.config();
		List<Runnable> steps = new ArrayList<>();
		steps.add(() -> {
			command("time set 6000");
			command("weather clear");
			command("tp @s ~ 120 ~ 0 20");
			command("fill ~-4 119 ~-3 ~4 119 ~7 minecraft:stone");
			command("fill ~-4 120 ~-3 ~4 124 ~7 minecraft:air");
			command("scoreboard objectives add arctic dummy");
			command("scoreboard objectives setdisplay list arctic");
			c.badgeOthersOnly = false;
			c.ownNametag = true;
			c.badgeNametags = true;
			c.badgeWhere = 0;
			camera(true);
		});
		// {style, scale, fancy}
		int[][] views = {{0, 2, 0}, {1, 2, 0}, {2, 2, 0}, {3, 2, 0}, {0, 1, 0}, {0, 3, 0}, {0, 4, 0}, {0, 2, 1}, {0, 3, 1}};
		for (final int[] v : views) {
			steps.add(() -> {
				c.badgeStyle = v[0];
				boolean fancy = v[2] == 1;
				if (c.fancy != fancy) {
					c.fancy = fancy;
					//#if MC >= 26.1
					ArcticPacks.setSmoothFont(fancy);
					//#else
					SmoothFont.changed();
					//#endif
				}
				scale(v[1]);
				Compat.setDown(Minecraft.getInstance().options.keyPlayerList, true);
			});
			steps.add(() -> WorldTest.shot("badge-tab-s" + v[0] + "-g" + v[1] + "-f" + v[2]));
			steps.add(() -> Compat.setDown(Minecraft.getInstance().options.keyPlayerList, false));
			steps.add(() -> WorldTest.shot("badge-tag-s" + v[0] + "-g" + v[1] + "-f" + v[2]));
		}
		steps.add(() -> {
			ArcticMod.LOG.info("badgetest: done");
			Minecraft.getInstance().stop();
		});
		for (int i = 0; i < steps.size(); i++) {
			final Runnable step = steps.get(i);
			TIMER.schedule(() -> run(step), (long) i * STEP, TimeUnit.SECONDS);
		}
	}
}
