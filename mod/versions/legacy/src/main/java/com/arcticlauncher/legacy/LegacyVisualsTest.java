package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.GameMenuScreen;

/**
 * Development check, only with {@code -Darctic.selftest=visuals}: dropped items with and without item
 * physics, then the camera turning fast with and without motion blur. Screenshots are named in the log.
 */
final class LegacyVisualsTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-visualstest");
		t.setDaemon(true);
		return t;
	});
	private static final int STEP_SECONDS = 4;
	private static final long TURN_EVERY_MS = 10;
	private static final float TURN_BY = 30f;
	//#if MC >= 1.11
	private static final String ITEM = "item";
	//#else
	private static final String ITEM = "Item";
	//#endif

	private LegacyVisualsTest() {}

	static boolean requested() {
		return "visuals".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticLegacy.LOG.info("visualstest: scheduled");
		TIMER.schedule(() -> run(() -> LegacyWorldTest.createTestWorld(LegacyVisualsTest::steps)), 10, TimeUnit.SECONDS);
	}

	/** Next frame; the test window may lose focus, so a pause menu that brings is closed first. */
	private static void run(Runnable r) {
		LegacyHooks.onNextFrame(() -> {
			MinecraftClient mc = MinecraftClient.getInstance();
			if (mc.currentScreen instanceof GameMenuScreen) {
				mc.setScreen(null);
			}
			r.run();
		});
	}

	private static void summon(String id, String at, int count) {
		LegacyWorldTest.command("summon " + ITEM + " " + at + " {Item:{id:minecraft:" + id + ",Count:" + count + "},PickupDelay:32767}");
	}

	private static void steps() {
		final ClientConfig c = ArcticClient.config();
		Runnable[] steps = {
				() -> {
					LegacyWorldTest.command("time set 6000");
					LegacyWorldTest.command("weather clear");
					// A stone platform in open sky, whatever the world generated around the spawn.
					LegacyWorldTest.command("tp @p ~ 120 ~ 0 50");
					LegacyWorldTest.command("fill ~-4 119 ~-3 ~4 119 ~7 minecraft:stone");
					LegacyWorldTest.command("fill ~-4 120 ~-3 ~4 124 ~7 minecraft:air");
					summon("diamond", "~ ~1 ~2", 1);
					summon("iron_sword", "~1 ~1 ~2.5", 1);
					summon("stone", "~-1 ~1 ~2.5", 1);
					summon("apple", "~0.5 ~1 ~3.5", 64);
					c.itemPhysics = false;
				},
				() -> LegacyWorldTest.shot("items-vanilla"),
				() -> c.itemPhysics = true,
				() -> LegacyWorldTest.shot("items-physics"),
				() -> {
					c.motionBlur = false;
					spin("blur-off");
				},
				() -> {
					c.motionBlur = true;
					c.motionBlurStrength = 4;
					spin("blur-max");
				},
				() -> {
					c.motionBlurStrength = 2;
					spin("blur-medium");
				},
				() -> {
					c.motionBlur = false;
					c.itemPhysics = false;
					ArcticLegacy.LOG.info("visualstest: done");
					MinecraftClient.getInstance().scheduleStop();
				},
		};
		for (int i = 0; i < steps.length; i++) {
			final Runnable step = steps[i];
			TIMER.schedule(() -> run(step), (long) i * STEP_SECONDS, TimeUnit.SECONDS);
		}
	}

	/** Turn the camera fast for a moment and screenshot while it turns. */
	private static void spin(final String name) {
		final ScheduledFuture<?> turning = TIMER.scheduleAtFixedRate(() -> run(() -> {
			MinecraftClient mc = MinecraftClient.getInstance();
			if (mc.player != null) {
				mc.player.increaseTransforms(TURN_BY, 0f);
			}
		}), 0, TURN_EVERY_MS, TimeUnit.MILLISECONDS);
		TIMER.schedule(() -> run(() -> LegacyWorldTest.shot(name)), 1500, TimeUnit.MILLISECONDS);
		TIMER.schedule(() -> turning.cancel(false), 1700, TimeUnit.MILLISECONDS);
	}
}
