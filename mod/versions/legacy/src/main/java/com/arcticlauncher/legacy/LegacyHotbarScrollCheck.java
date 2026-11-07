package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;

/**
 * {@code -Darctic.selftest=hotbarscroll} on 1.8.9–1.12.2: the game's own
 * {@code scrollInHotbar} ({@code InventoryPlayer.changeCurrentItem}). Off stops on slot 1 and 9;
 * on wraps. The log is the result.
 */
final class LegacyHotbarScrollCheck {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-hotbarscroll");
		t.setDaemon(true);
		return t;
	});

	private LegacyHotbarScrollCheck() {}

	static boolean requested() {
		return "hotbarscroll".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticLegacy.LOG.info("hotbarscroll: scheduled");
		// A window that isn't focused opens a screen that pauses the world; the load wait needs it gone.
		TIMER.scheduleAtFixedRate(() -> MinecraftClient.getInstance().submit(() -> {
			MinecraftClient mc = MinecraftClient.getInstance();
			if (mc.world != null && mc.player != null && mc.currentScreen != null) {
				mc.setScreen(null);
			}
		}), 8, 1, TimeUnit.SECONDS);
		TIMER.schedule(() -> MinecraftClient.getInstance().submit(() -> LegacyWorldTest.createTestWorld(LegacyHotbarScrollCheck::run)),
				10, TimeUnit.SECONDS);
	}

	private static void run() {
		ClientConfig config = ArcticClient.config();
		config.hotbarScrollWrap = false;
		boolean clamped = step("clamp", 0, 1, 0) && step("clamp", 8, -1, 8) && step("clamp", 3, 1, 2);
		config.hotbarScrollWrap = true;
		boolean wrapped = step("wrap", 0, 1, 8) && step("wrap", 8, -1, 0) && step("wrap", 3, 1, 2);
		LegacyWorldTest.shot("hotbar-scroll");
		if (clamped && wrapped) {
			ArcticLegacy.LOG.info("hotbarscroll: PASSED");
		} else {
			ArcticLegacy.LOG.error("hotbarscroll: FAILED clamp {} wrap {}", clamped, wrapped);
		}
		MinecraftClient.getInstance().scheduleStop();
	}

	/** A positive amount moves toward slot 1, as {@code changeCurrentItem} does. */
	private static boolean step(String label, int from, int amount, int want) {
		MinecraftClient mc = MinecraftClient.getInstance();
		mc.setScreen(null);
		mc.player.inventory.selectedSlot = from;
		mc.player.inventory.scrollInHotbar(amount);
		int got = mc.player.inventory.selectedSlot;
		ArcticLegacy.LOG.info("hotbarscroll: {} from {} wheel {} -> {} (want {})", label, from, amount, got, want);
		return got == want;
	}
}
