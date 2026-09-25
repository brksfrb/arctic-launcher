package com.arcticlauncher.mod;

import java.util.UUID;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Screenshot;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.resources.Identifier;

/**
 * Development check, only with {@code -Darctic.selftest=true}: loads the
 * mixin targets, waits for the player's cape, opens the Arctic menu,
 * saves a screenshot and quits. Lets the mod be verified without clicking.
 */
final class SelfTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-selftest");
		t.setDaemon(true);
		return t;
	});

	private SelfTest() {}

	static void maybeStart() {
		if (!Boolean.getBoolean("arctic.selftest")) {
			return;
		}
		ArcticMod.LOG.info("selftest: scheduled");
		TIMER.schedule(SelfTest::loadTargets, 8, TimeUnit.SECONDS);
	}

	private static void loadTargets() {
		ClassLoader loader = SelfTest.class.getClassLoader();
		for (String name : new String[] {
				"net.minecraft.client.player.AbstractClientPlayer",
				"net.minecraft.client.gui.screens.PauseScreen",
				"net.minecraft.client.gui.screens.TitleScreen"}) {
			try {
				Class.forName(name, false, loader);
				ArcticMod.LOG.info("selftest: mixin target {} loaded", name);
			} catch (Throwable e) {
				ArcticMod.LOG.error("selftest: FAILED to load {}", name, e);
			}
		}
		waitForCape(0);
	}

	private static void waitForCape(int attempt) {
		UUID self = Minecraft.getInstance().getUser().getProfileId();
		Identifier cape = Cosmetics.capeFor(self);
		if (cape != null) {
			ArcticMod.LOG.info("selftest: cape texture {}", cape);
			openMenu();
		} else if (attempt < 20) {
			TIMER.schedule(() -> waitForCape(attempt + 1), 1, TimeUnit.SECONDS);
		} else {
			ArcticMod.LOG.error("selftest: FAILED no cape for {}", self);
			openMenu();
		}
	}

	private static void openMenu() {
		Minecraft mc = Minecraft.getInstance();
		mc.execute(() -> {
			Screen parent = mc.gui.screen();
			mc.gui.setScreen(new ArcticScreen(parent));
		});
		TIMER.schedule(() -> mc.execute(() -> {
			Screenshot.grab(mc, false);
			ArcticMod.LOG.info("selftest: screenshot taken");
		}), 5, TimeUnit.SECONDS);
		TIMER.schedule(() -> mc.execute(() -> {
			ArcticMod.LOG.info("selftest: done");
			mc.stop();
		}), 8, TimeUnit.SECONDS);
	}
}
