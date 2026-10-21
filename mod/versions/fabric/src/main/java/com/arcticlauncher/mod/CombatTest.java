package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/**
 * Self-test for Combat Feedback ({@code -Darctic.selftest=combat}): a fresh world, a zombie that
 * doesn't move, a real attack through the game's own attack call (so the hook is the one players
 * use), the zombie's death, then low health. Screenshots go to the game's screenshots folder; the
 * log says what the feature saw.
 */
final class CombatTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-combattest");
		t.setDaemon(true);
		return t;
	});
	private static final int STEP_SECONDS = 4;
	private static final long AFTER_HIT_MS = 80;
	private static final long AFTER_KILL_MS = 300;
	private static final long KILL_AFTER_MS = 1000;
	private static final long LOW_SHOT_MS = 250;

	private CombatTest() {}

	static boolean requested() {
		return "combat".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticMod.LOG.info("combattest: scheduled");
		TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(CombatTest::steps)), 10, TimeUnit.SECONDS);
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
					c.hitMarker = true;
					c.hitSound = 1;
					c.killSound = 3;
					c.lowHealthWarning = true;
					c.lowHealthHearts = 3;
					command("difficulty easy");
					command("gamemode survival");
					command("time set noon");
					command("weather clear");
					command("tp @s ~ 200 ~ 0 20");
					command("fill ~-4 199 ~-3 ~4 199 ~7 minecraft:stone");
					command("fill ~-4 200 ~-3 ~4 204 ~7 minecraft:air");
					command("summon zombie ~ ~ ~2 {NoAI:1b,PersistenceRequired:1b,Silent:1b}");
				},
				() -> {
					net.minecraft.world.entity.Entity zombie = zombie();
					ArcticMod.LOG.info("combattest: zombie {} dead before hit {}", zombie != null, ArcticClient.platform().lastTargetDead());
					if (zombie != null) {
						Minecraft mc = Minecraft.getInstance();
						mc.gameMode.attack(mc.player, zombie);
						TIMER.schedule(() -> run(() -> WorldTest.shot("combat-hit")), AFTER_HIT_MS, TimeUnit.MILLISECONDS);
						// The kill follows within the window in which a death counts as yours.
						TIMER.schedule(() -> run(() -> command("kill @e[type=zombie]")), KILL_AFTER_MS, TimeUnit.MILLISECONDS);
						TIMER.schedule(() -> run(() -> WorldTest.shot("combat-kill")), KILL_AFTER_MS + AFTER_KILL_MS, TimeUnit.MILLISECONDS);
					}
				},
				() -> {
					ArcticMod.LOG.info("combattest: target dead after kill {}", ArcticClient.platform().lastTargetDead());
					command("damage @s 19");
					// Health comes back fast on easy: the picture is taken right after the damage.
					TIMER.schedule(() -> run(() -> {
						float[] health = ArcticClient.platform().health();
						ArcticMod.LOG.info("combattest: health {} of {}", health == null ? -1 : health[0], health == null ? -1 : health[1]);
						WorldTest.shot("combat-low-health");
					}), LOW_SHOT_MS, TimeUnit.MILLISECONDS);
				},
				() -> {
					c.hitMarker = false;
					c.hitSound = 0;
					c.killSound = 0;
					c.lowHealthWarning = false;
					ArcticMod.LOG.info("combattest: done");
					Minecraft.getInstance().stop();
				},
		};
		for (int i = 0; i < steps.length; i++) {
			Runnable step = steps[i];
			TIMER.schedule(() -> run(step), (long) i * STEP_SECONDS, TimeUnit.SECONDS);
		}
	}

	private static net.minecraft.world.entity.Entity zombie() {
		//#if MC >= 1.14
		for (net.minecraft.world.entity.Entity e : Minecraft.getInstance().level.entitiesForRendering()) {
		//#else
		for (net.minecraft.world.entity.Entity e : Minecraft.getInstance().level.loadedEntities) {
		//#endif
			if (e.getType().toString().endsWith("zombie")) {
				return e;
			}
		}
		return null;
	}
}
