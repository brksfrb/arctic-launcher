package com.arcticlauncher.mod;

//#if MC >= 26.3
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.hud.HudWidget;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Screenshot;
import net.minecraft.network.chat.Component;
import net.minecraft.world.Difficulty;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.LevelSettings;
import net.minecraft.world.level.WorldDataConfiguration;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.levelgen.presets.WorldPresets;
//#endif

/**
 * Development check, only with {@code -Darctic.selftest=world} (26.3):
 * creates a creative world, turns every feature on, takes screenshots of
 * each (HUD, zoom, freelook, fullbright at night, clear weather, chat),
 * then quits. Verifies the game features without a keyboard.
 */
final class WorldTest {
	private WorldTest() {}

	static boolean requested() {
		return "world".equals(System.getProperty("arctic.selftest"));
	}

	//#if MC >= 26.3
	private static final int STEP = 4;
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-worldtest");
		t.setDaemon(true);
		return t;
	});

	static void start() {
		ArcticMod.LOG.info("worldtest: scheduled");
		TIMER.schedule(() -> run(WorldTest::createWorld), 10, TimeUnit.SECONDS);
	}

	private static void run(Runnable r) {
		Minecraft.getInstance().execute(r);
	}

	private static void createWorld() {
		Minecraft mc = Minecraft.getInstance();
		String name = "Arctic Selftest " + System.currentTimeMillis();
		LevelSettings settings = new LevelSettings(name, GameType.CREATIVE,
				new LevelSettings.DifficultySettings(Difficulty.PEACEFUL, false, false), true, WorldDataConfiguration.DEFAULT);
		ArcticMod.LOG.info("worldtest: creating {}", name);
		mc.createWorldOpenFlows().createFreshLevel(name, settings, WorldOptions.defaultWithRandomSeed(),
				WorldPresets::createNormalWorldDimensions, Compat.screen());
		waitForWorld(0);
	}

	private static void waitForWorld(int attempt) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.level != null && mc.player != null && Compat.screen() == null) {
			ArcticMod.LOG.info("worldtest: in the world after {}s", attempt);
			TIMER.schedule(() -> run(WorldTest::tour), 8, TimeUnit.SECONDS);
		} else if (attempt < 120) {
			TIMER.schedule(() -> waitForWorld(attempt + 1), 1, TimeUnit.SECONDS);
		} else {
			ArcticMod.LOG.error("worldtest: FAILED world never loaded");
			run(mc::stop);
		}
	}

	private static void tour() {
		ClientConfig c = ArcticClient.config();
		// The test window may lose focus; keep the game running and unpaused.
		Minecraft.getInstance().options.pauseOnLostFocus = false;
		for (HudWidget w : ArcticClient.hud().widgets()) {
			HudSlot slot = ArcticClient.hud().slot(w);
			slot.enabled = true;
		}
		c.crosshair.enabled = true;
		c.crosshair.style = "cross-dot";
		c.crosshair.color = 0xFF7DD3FC;
		c.chatTimestamps = true;
		c.chatStack = true;
		Runnable[] steps = {
				() -> {
					for (int i = 0; i < 3; i++) {
						Minecraft.getInstance().player.sendSystemMessage(Component.literal("Arctic test line"));
					}
					later(() -> shot("world-hud-chat"));
				},
				() -> {
					ArcticClient.features().simulate(true, false);
					later(() -> shot("world-zoom"));
				},
				() -> {
					ArcticClient.features().simulate(false, true);
					ArcticClient.features().turn(600, 0);
					later(() -> shot("world-freelook"));
				},
				() -> {
					ArcticClient.features().simulate(false, false);
					command("time set midnight");
					c.fullbright = false;
					later(() -> shot("world-night"));
				},
				() -> {
					c.fullbright = true;
					later(() -> shot("world-night-fullbright"));
				},
				() -> {
					c.fullbright = false;
					command("time set noon");
					command("weather rain");
					c.clearWeather = false;
					later(() -> shot("world-rain"));
				},
				() -> {
					c.clearWeather = true;
					later(() -> shot("world-rain-cleared"));
				},
				() -> {
					c.clearWeather = false;
					command("weather clear");
					ArcticClient.platform().openPage(new com.arcticlauncher.client.menu.HudEditor());
					TIMER.schedule(() -> run(() -> shot("world-hud-editor")), STEP, TimeUnit.SECONDS);
				},
				() -> {
					ArcticMod.LOG.info("worldtest: done");
					Minecraft.getInstance().stop();
				}};
		for (int i = 0; i < steps.length; i++) {
			Runnable step = steps[i];
			TIMER.schedule(() -> run(step), (long) i * STEP * 2, TimeUnit.SECONDS);
		}
	}

	private static void command(String command) {
		Minecraft.getInstance().player.connection.sendCommand(command);
	}

	private static void later(Runnable r) {
		TIMER.schedule(() -> run(() -> {
			Compat.setScreen(null);
			r.run();
		}), STEP, TimeUnit.SECONDS);
	}

	private static void shot(String name) {
		Screenshot.grab(Minecraft.getInstance(), false);
		ArcticMod.LOG.info("worldtest: screenshot {}", name);
	}
	//#else
	static void start() {
		ArcticMod.LOG.warn("worldtest: needs 26.3");
	}
	//#endif
}
