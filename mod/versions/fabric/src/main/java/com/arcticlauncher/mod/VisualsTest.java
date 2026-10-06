//#if MC >= 26.1
package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/**
 * Self-test for item physics and motion blur ({@code -Darctic.selftest=visuals}): a fresh world, dropped
 * items with and without item physics, then the camera turning fast with and without motion blur.
 * Screenshots go to the game's screenshots folder; the log names each one.
 */
final class VisualsTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-visualstest");
		t.setDaemon(true);
		return t;
	});
	private static final int STEP_SECONDS = 4;
	/** How often the camera turns during the spin, and how far (mouse units). */
	private static final long TURN_EVERY_MS = 10;
	private static final double TURN_BY = 30;

	private VisualsTest() {}

	static boolean requested() {
		return "visuals".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticMod.LOG.info("visualstest: scheduled");
		TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(VisualsTest::steps)), 10, TimeUnit.SECONDS);
	}

	/** On the game thread; the test window may lose focus, so a pause menu that brings is closed first. */
	private static void run(Runnable r) {
		Minecraft.getInstance().execute(() -> {
			if (Compat.screen() instanceof net.minecraft.client.gui.screens.PauseScreen) {
				Compat.setScreen(null);
			}
			r.run();
		});
	}

	private static void command(String command) {
		Compat.sendChat("/" + command);
	}

	private static void steps() {
		ClientConfig c = ArcticClient.config();
		Runnable[] steps = {
				() -> {
					command("time set noon");
					command("weather clear");
					// A stone platform in open sky, whatever the world generated around the spawn.
					command("tp @s ~ 200 ~ 0 50");
					command("fill ~-4 199 ~-3 ~4 199 ~7 minecraft:stone");
					command("fill ~-4 200 ~-3 ~4 204 ~7 minecraft:air");
					// A flat item, a tool, a block and a full stack, a little in front of the player.
					command("summon item ~ ~1 ~2 {Item:{id:\"minecraft:diamond\",count:1},PickupDelay:32767}");
					command("summon item ~1 ~1 ~2.5 {Item:{id:\"minecraft:iron_sword\",count:1},PickupDelay:32767}");
					command("summon item ~-1 ~1 ~2.5 {Item:{id:\"minecraft:stone\",count:1},PickupDelay:32767}");
					command("summon item ~0.5 ~1 ~3.5 {Item:{id:\"minecraft:apple\",count:64},PickupDelay:32767}");
					c.itemPhysics = false;
				},
				() -> WorldTest.shot("items-vanilla"),
				() -> {
					c.itemPhysics = true;
				},
				() -> WorldTest.shot("items-physics"),
				() -> {
					command("summon item ~ ~4 ~2 {Item:{id:\"minecraft:gold_ingot\",count:1},PickupDelay:32767,Motion:[0.0,0.6,0.0]}");
					TIMER.schedule(() -> run(() -> WorldTest.shot("items-physics-air")), 300, TimeUnit.MILLISECONDS);
				},
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
					ArcticMod.LOG.info("visualstest: done");
					Minecraft.getInstance().stop();
				},
		};
		for (int i = 0; i < steps.length; i++) {
			Runnable step = steps[i];
			TIMER.schedule(() -> run(step), (long) i * STEP_SECONDS, TimeUnit.SECONDS);
		}
	}

	/** Turn the camera fast for a moment and screenshot while it turns. */
	private static void spin(String name) {
		ScheduledFuture<?> turning = TIMER.scheduleAtFixedRate(() -> run(() -> {
			if (Minecraft.getInstance().player != null) {
				Minecraft.getInstance().player.turn(TURN_BY, 0);
			}
		}), 0, TURN_EVERY_MS, TimeUnit.MILLISECONDS);
		TIMER.schedule(() -> run(() -> WorldTest.shot(name)), 1500, TimeUnit.MILLISECONDS);
		TIMER.schedule(() -> turning.cancel(false), 1700, TimeUnit.MILLISECONDS);
	}
}
//#endif
