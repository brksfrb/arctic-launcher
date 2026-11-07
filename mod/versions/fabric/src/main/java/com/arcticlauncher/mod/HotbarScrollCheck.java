package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/**
 * {@code -Darctic.selftest=hotbarscroll}: a world, then the game's own scroll. With wrap-around
 * off, slot 1 and slot 9 stay put; with it on, they wrap. The log is the result.
 */
final class HotbarScrollCheck {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-hotbarscroll");
		t.setDaemon(true);
		return t;
	});

	private HotbarScrollCheck() {}

	static boolean requested() {
		return "hotbarscroll".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticMod.LOG.info("hotbarscroll: scheduled");
		// A window that isn't focused opens a screen that pauses the world; the load wait needs it gone.
		TIMER.scheduleAtFixedRate(() -> Minecraft.getInstance().execute(() -> {
			Minecraft mc = Minecraft.getInstance();
			if (mc.player != null && mc.level != null) {
				Compat.setScreen(null);
			}
		}), 8, 1, TimeUnit.SECONDS);
		TIMER.schedule(() -> Minecraft.getInstance().execute(() -> WorldTest.createTestWorld(HotbarScrollCheck::run)), 10, TimeUnit.SECONDS);
	}

	private static void run() {
		ClientConfig config = ArcticClient.config();
		config.hotbarScrollWrap = false;
		boolean clamped = step("clamp", 0, 1, 0) && step("clamp", 8, -1, 8) && step("clamp", 3, 1, 2);
		config.hotbarScrollWrap = true;
		boolean wrapped = step("wrap", 0, 1, 8) && step("wrap", 8, -1, 0) && step("wrap", 3, 1, 2);
		WorldTest.shot("hotbar-scroll");
		if (clamped && wrapped) {
			ArcticMod.LOG.info("hotbarscroll: PASSED");
		} else {
			ArcticMod.LOG.error("hotbarscroll: FAILED clamp {} wrap {}", clamped, wrapped);
		}
		Minecraft.getInstance().stop();
	}

	/** A positive wheel moves toward slot 1, the same way the game's scroll does. */
	private static boolean step(String label, int from, double wheel, int want) {
		Minecraft mc = Minecraft.getInstance();
		Compat.setScreen(null);
		Compat.selectSlot(mc.player, from);
		((com.arcticlauncher.mod.mixin.MouseHandlerInvoker) mc.mouseHandler).arctic$scroll(Compat.windowHandle(), 0, wheel);
		int got = Compat.selectedSlot(mc.player);
		ArcticMod.LOG.info("hotbarscroll: {} from {} wheel {} -> {} (want {})", label, from, wheel, got, want);
		return got == want;
	}
}
