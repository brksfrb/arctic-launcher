package com.arcticlauncher.legacy;

import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

import net.minecraft.client.MinecraftClient;

/**
 * Development check, only with {@code -Darctic.selftest=keys}: makes a test
 * world, puts a Right Shift press into LWJGL's keyboard queue (as if typed,
 * so the game handles it in its usual order) and reports which screen is
 * open every second: the Arctic menu should open and stay. On 1.8 it also
 * feeds a sign update for a sign that isn't loaded, which must not reach
 * the chat. Quits after 20 seconds.
 */
final class LegacyKeysTest {
	private static final int SECONDS = 20;
	private static final int PRESS_AT = 3;
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-keytest");
		t.setDaemon(true);
		return t;
	});

	private LegacyKeysTest() {}

	static boolean requested() {
		return "keys".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> LegacyWorldTest.createTestWorld(LegacyKeysTest::watch)), 10, TimeUnit.SECONDS);
	}

	private static void watch() {
		MinecraftClient mc = MinecraftClient.getInstance();
		//#if MC < 1.9
		mc.getNetworkHandler().onUpdateSign(new net.minecraft.network.packet.s2c.play.UpdateSignS2CPacket(mc.world,
				new net.minecraft.util.math.BlockPos(100000, 64, 100000),
				new net.minecraft.text.Text[] {new net.minecraft.text.LiteralText("a"), new net.minecraft.text.LiteralText("b"),
						new net.minecraft.text.LiteralText("c"), new net.minecraft.text.LiteralText("d")}));
		ArcticLegacy.LOG.info("keytest: fed a sign update for an unloaded sign");
		//#endif
		ArcticLegacy.LOG.info("keytest: ready");
		for (int s = 1; s <= SECONDS; s++) {
			final int at = s;
			TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> {
				Object screen = MinecraftClient.getInstance().currentScreen;
				ArcticLegacy.LOG.info("keytest: {}s screen {}", at, screen == null ? "none" : screen.getClass().getName());
				if (at == PRESS_AT) {
					type(org.lwjgl.input.Keyboard.KEY_RSHIFT);
					ArcticLegacy.LOG.info("keytest: pressed Right Shift");
				}
				if (at == SECONDS - 2) {
					LegacyWorldTest.shot("keytest");
				}
				if (at == SECONDS) {
					ArcticLegacy.LOG.info("keytest: done");
					MinecraftClient.getInstance().scheduleStop();
				}
			}), s, TimeUnit.SECONDS);
		}
	}

	/** A key press and release in LWJGL's event queue, read by the game's next keyboard step. */
	private static void type(int key) {
		try {
			java.lang.reflect.Field field = org.lwjgl.input.Keyboard.class.getDeclaredField("readBuffer");
			field.setAccessible(true);
			java.nio.ByteBuffer events = (java.nio.ByteBuffer) field.get(null);
			long now = System.nanoTime();
			events.compact();
			for (int down = 1; down >= 0; down--) {
				events.putInt(key).put((byte) down).putInt(0).putLong(now + (1 - down)).put((byte) 0);
			}
			events.flip();
		} catch (ReflectiveOperationException e) {
			ArcticLegacy.LOG.error("keytest: can't type", e);
		}
	}
}
