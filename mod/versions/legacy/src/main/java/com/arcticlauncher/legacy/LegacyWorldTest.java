package com.arcticlauncher.legacy;

import java.io.File;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.Mentions;
import com.arcticlauncher.client.feature.Streamer;
import com.arcticlauncher.client.hud.HudWidget;
import com.arcticlauncher.client.menu.Menus;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.GameMenuScreen;
import net.minecraft.client.util.ScreenshotUtils;
import net.minecraft.text.LiteralText;
import net.minecraft.world.level.LevelGeneratorType;
import net.minecraft.world.level.LevelInfo;

/**
 * Development check, only with {@code -Darctic.selftest=world}: makes a
 * creative world, turns the features on one by one with a screenshot of
 * each, then quits. The 1.8.9 twin of the modern WorldTest.
 */
final class LegacyWorldTest {
	private static final int STEP = 8;
	private static final String PREFIX = "Arctic Selftest ";
	/** Entity ids went lowercase in 1.11. */
	//#if MC >= 1.11
	private static final String PIG = "pig";
	//#else
	private static final String PIG = "Pig";
	//#endif
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-worldtest");
		t.setDaemon(true);
		return t;
	});

	private LegacyWorldTest() {}

	static boolean requested() {
		return "world".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticLegacy.LOG.info("worldtest: scheduled");
		TIMER.schedule(() -> run(() -> createTestWorld(LegacyWorldTest::tour)), 10, TimeUnit.SECONDS);
	}

	private static void run(Runnable r) {
		LegacyHooks.onNextFrame(r);
	}

	/** A fresh creative test world; {@code then} runs (game thread) a few seconds after it's loaded. */
	static void createTestWorld(Runnable then) {
		MinecraftClient mc = MinecraftClient.getInstance();
		deleteOld(new File(mc.runDirectory, "saves"));
		String name = PREFIX + System.currentTimeMillis();
		//#if MC >= 1.10
		LevelInfo info = new LevelInfo(System.currentTimeMillis(), net.minecraft.world.GameMode.CREATIVE, true, false, LevelGeneratorType.DEFAULT)
		//#else
		LevelInfo info = new LevelInfo(System.currentTimeMillis(), LevelInfo.GameMode.CREATIVE, true, false, LevelGeneratorType.DEFAULT)
		//#endif
				.enableCommands();
		ArcticLegacy.LOG.info("worldtest: creating {}", name);
		mc.startIntegratedServer(name, name, info);
		waitForWorld(0, then);
	}

	private static void waitForWorld(int attempt, Runnable then) {
		MinecraftClient mc = MinecraftClient.getInstance();
		if (mc.world != null && mc.player != null && mc.currentScreen == null) {
			ArcticLegacy.LOG.info("worldtest: in the world after {}s", attempt);
			TIMER.schedule(() -> run(then), 6, TimeUnit.SECONDS);
		} else if (attempt < 120) {
			TIMER.schedule(() -> waitForWorld(attempt + 1, then), 1, TimeUnit.SECONDS);
		} else {
			ArcticLegacy.LOG.error("worldtest: FAILED world never loaded");
		}
	}

	static void command(String c) {
		MinecraftClient.getInstance().player.sendChatMessage("/" + c);
	}

	private static void tour() {
		final MinecraftClient mc = MinecraftClient.getInstance();
		Runnable[] steps = {
				() -> {
					for (HudWidget w : ArcticClient.hud().widgets()) {
						ArcticClient.hud().slot(w).enabled = true;
					}
					ArcticClient.saveConfig();
					ArcticLegacy.LOG.info("worldtest: fps {} light {} biome {} server {} dim {}", ArcticClient.platform().fps(),
							java.util.Arrays.toString(ArcticClient.platform().light()), ArcticClient.platform().biome(),
							ArcticClient.platform().server(), ArcticClient.platform().dimension());
					later(() -> shot("world-hud"));
				},
				() -> {
					ArcticClient.features().simulate(true, false);
					later(() -> {
						shot("zoom");
						ArcticClient.features().simulate(false, false);
					});
				},
				() -> {
					ArcticClient.features().simulate(false, true);
					later(() -> {
						shot("freelook");
						ArcticClient.features().simulate(false, false);
					});
				},
				() -> {
					ArcticClient.config().streamerMode = true;
					String me = mc.getSession().getUsername();
					ArcticLegacy.LOG.info("worldtest: streamer mask '{}' -> '{}'", "hi " + me, Streamer.mask("hi " + me));
					mc.inGameHud.getChatHud().addMessage(new LiteralText("<" + me + "> can you see my name?"));
					later(() -> {
						shot("streamer");
						ArcticClient.config().streamerMode = false;
					});
				},
				() -> {
					// Up in the air on a glass block, a pig right in front; the crosshair shows what you aim at.
					command("tp @p ~ 250 ~ 0 0");
					command("setblock ~ 249 ~ glass");
					command("summon " + PIG + " ~ 250.9 ~3 {NoAI:1b}");
					ArcticClient.config().crosshair.enabled = true;
					ArcticClient.config().crosshair.targetColors = true;
					TIMER.schedule(() -> run(() -> {
						ArcticLegacy.LOG.info("worldtest: aiming at kind {}", ArcticClient.platform().aimKind());
						shot("crosshair-target");
						ArcticClient.config().hitColor = 0x7DD3FC;
						command("effect @e[type=" + PIG + "] 7 1 0");
					}), 4, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> shot("hit-color")), 4300, TimeUnit.MILLISECONDS);
				},
				() -> {
					// Night on screen, a thick white outline on the glass, a scoreboard without numbers, typed placeholders.
					ArcticClient.config().timeLock = 18000;
					ArcticClient.config().outlineColor = 0xFFFFFFFF;
					ArcticClient.config().outlineWidth = 3f;
					ArcticClient.config().scoreboardNumbers = false;
					ArcticClient.config().scoreboardBackground = false;
					command("scoreboard objectives add arctic dummy Arctic");
					command("scoreboard objectives setdisplay sidebar arctic");
					command("scoreboard players set Steve arctic 42");
					mc.player.sendChatMessage("typed {x} {y} {z}");
					mc.player.pitch = 90f;
					later(() -> shot("night-outline-scoreboard"));
				},
				() -> {
					ArcticClient.config().timeLock = -1;
					mc.player.pitch = 0f;
					ArcticClient.waypoints().add(ArcticClient.platform(), "Home");
					command("tp @p ~40 ~ ~60 180 0");
					TIMER.schedule(() -> run(() -> {
						ArcticLegacy.LOG.info("worldtest: waypoints here {} in {} of {}", ArcticClient.waypoints().here(ArcticClient.platform()).size(),
								ArcticClient.platform().dimension(), ArcticClient.platform().worldKey());
						shot("waypoints");
					}), 3, TimeUnit.SECONDS);
				},
				() -> {
					String me = mc.getSession().getUsername();
					ArcticLegacy.LOG.info("worldtest: mentions friend {} own {}", Mentions.mentions("<Steve> gg " + me, me),
							Mentions.mentions("<" + me + "> hi", me));
					mc.inGameHud.getChatHud().addMessage(new LiteralText("<Steve> gg " + me + ", rematch?"));
					command("give @p iron_sword 1 246");
					TIMER.schedule(() -> run(() -> shot("mention-durability")), 3, TimeUnit.SECONDS);
				},
				() -> {
					Menus.select("world");
					ArcticClient.platform().openPage(Menus.selected());
					TIMER.schedule(() -> run(() -> shot("menu-world")), 2, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> mc.setScreen(new GameMenuScreen())), 4, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> shot("pause")), 6, TimeUnit.SECONDS);
				},
				() -> {
					ArcticLegacy.LOG.info("worldtest: done");
					mc.scheduleStop();
				},
		};
		for (int i = 0; i < steps.length; i++) {
			final Runnable step = steps[i];
			TIMER.schedule(() -> run(step), (long) i * STEP, TimeUnit.SECONDS);
		}
	}

	private static void later(Runnable r) {
		TIMER.schedule(() -> run(r), 3, TimeUnit.SECONDS);
	}

	static void shot(String name) {
		MinecraftClient mc = MinecraftClient.getInstance();
		ScreenshotUtils.saveScreenshot(mc.runDirectory, mc.width, mc.height, mc.getFramebuffer());
		ArcticLegacy.LOG.info("worldtest: screenshot {}", name);
	}

	/** Earlier runs' worlds (only ours, by name), so test runs don't pile up. */
	private static void deleteOld(File saves) {
		deleteWorlds(saves, PREFIX);
	}

	/** Worlds of ours whose folder starts with {@code prefix} (test worlds, old duel arenas). */
	static void deleteWorlds(File saves, String prefix) {
		File[] dirs = saves.listFiles();
		if (dirs == null) {
			return;
		}
		for (File dir : dirs) {
			if (dir.isDirectory() && dir.getName().startsWith(prefix)) {
				try (java.util.stream.Stream<java.nio.file.Path> files = java.nio.file.Files.walk(dir.toPath())) {
					files.sorted(java.util.Comparator.reverseOrder()).forEach(f -> f.toFile().delete());
				} catch (java.io.IOException e) {
					ArcticLegacy.LOG.warn("couldn't delete the world {}", dir.getName());
				}
			}
		}
	}
}
