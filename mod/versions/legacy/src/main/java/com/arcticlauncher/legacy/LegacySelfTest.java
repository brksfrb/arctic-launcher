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
	private static final String[] TABS = {"mods", "hud.fps", "hud.keystrokes", "crosshair", "packs", "style"};
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
		// Loads the player class (its mixins apply now) and looks up a tested player's tiers.
		final java.util.UUID tested = java.util.UUID.fromString("06ec3577-3299-45fa-bbdf-613b1f86c8ab");
		later(step, () -> {
			ArcticLegacy.LOG.info("selftest: player class {}", net.minecraft.entity.player.PlayerEntity.class.getName());
			ArcticClient.tiers().tag(tested, "ItzRealMe");
		});
		later(step++, () -> shot("title"));
		for (final String tab : TABS) {
			later(step++, () -> {
				Menus.select(tab);
				ArcticClient.platform().openPage(Menus.selected());
			});
			later(step++, () -> shot("menu-" + tab));
		}
		// The game's multiplayer list opens Arctic's in its place.
		later(step++, () -> MinecraftClient.getInstance().setScreen(
				new net.minecraft.client.gui.screen.multiplayer.MultiplayerScreen(new net.minecraft.client.gui.screen.TitleScreen())));
		later(step++, () -> {
			ArcticLegacy.LOG.info("selftest: multiplayer screen is {}", MinecraftClient.getInstance().currentScreen.getClass().getSimpleName());
			shot("servers");
		});
		later(step++, () -> ArcticClient.platform().openPage(new com.arcticlauncher.client.servers.DisconnectedPage("Connection Lost", "Timed out")));
		later(step++, () -> shot("disconnected"));
		// A scripted controller: the pointer onto the pause menu's "Back to Game", then A.
		final com.arcticlauncher.client.controller.Gamepad pad = new com.arcticlauncher.client.controller.Gamepad();
		later(step++, () -> MinecraftClient.getInstance().setScreen(new net.minecraft.client.gui.screen.GameMenuScreen()));
		later(step++, () -> {
			net.minecraft.client.gui.screen.Screen screen = MinecraftClient.getInstance().currentScreen;
			double scale = (double) org.lwjgl.opengl.Display.getWidth() / screen.width;
			com.arcticlauncher.client.controller.Controller.scripted = pad;
			ArcticClient.platform().setCursor(screen.width / 2.0 * scale, (screen.height / 4.0 + 18) * scale);
		});
		// A frame later: a press only counts once the pad is connected.
		later(step++, () -> pad.buttons[com.arcticlauncher.client.controller.Gamepad.A] = true);
		later(step++, () -> {
			pad.buttons[com.arcticlauncher.client.controller.Gamepad.A] = false;
			net.minecraft.client.gui.screen.Screen screen = MinecraftClient.getInstance().currentScreen;
			ArcticLegacy.LOG.info("selftest: after clicking Back to Game {}", screen == null ? "none" : screen.getClass().getSimpleName());
			com.arcticlauncher.client.controller.Controller.scripted = null;
		});
		later(step, () -> {
			com.arcticlauncher.client.tiers.Tiers.Tag tag = ArcticClient.tiers().tag(tested, "ItzRealMe");
			ArcticLegacy.LOG.info("selftest: tier tag {}", tag == null ? "none" : tag.formatted());
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
