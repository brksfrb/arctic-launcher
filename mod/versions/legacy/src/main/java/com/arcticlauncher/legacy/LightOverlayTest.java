package com.arcticlauncher.legacy;
import com.arcticlauncher.client.ArcticClient;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;
/** Offline-only, opt-in proof scene: -Darctic.selftest=lightoverlay (dark cave, lit area, settings page, FPS off/on). */
final class LightOverlayTest {
	private static final java.util.concurrent.ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-lightoverlay-proof"); t.setDaemon(true); return t;
	});
	private static final java.util.List<Integer> FPS = new java.util.concurrent.CopyOnWriteArrayList<Integer>();
	private LightOverlayTest() {}
	static void start() {
		if (Boolean.getBoolean("arctic.lo.server")) {
			// On a local server (you're op there): the same scene once you're in its world.
			TIMER.schedule(() -> run(LightOverlayTest::awaitWorld), 10, TimeUnit.SECONDS);
			return;
		}
		TIMER.schedule(() -> run(() -> LegacyWorldTest.createTestWorld(LightOverlayTest::scene)), 10, TimeUnit.SECONDS);
	}
	private static void run(Runnable r) { LegacyHooks.onNextFrame(r); }
	private static void command(String s) { LegacyWorldTest.command(s); }
	private static void at(int seconds, Runnable r) { TIMER.schedule(() -> run(r), seconds, TimeUnit.SECONDS); }
	private static void awaitWorld() {
		MinecraftClient mc = MinecraftClient.getInstance();
		if (mc.world != null && mc.player != null && mc.currentScreen == null) {
			TIMER.schedule(() -> run(LightOverlayTest::scene), 6, TimeUnit.SECONDS);
		} else {
			TIMER.schedule(() -> run(LightOverlayTest::awaitWorld), 1, TimeUnit.SECONDS);
		}
	}
	private static void scene() {
		command("tp @p 0.5 220 -7 0 0");
		TIMER.schedule(() -> run(LightOverlayTest::build), 7, TimeUnit.SECONDS);
	}
	private static void build() {
		command("gamemode creative");
		// Lit area: open ground in daylight (yellow), under a roof away from torches (red), by torches (green).
		command("fill -14 219 -14 14 219 44 minecraft:stone");
		command("fill -14 220 -14 14 232 44 minecraft:air");
		command("fill -14 224 4 14 224 44 minecraft:stone");
		command("setblock 8 220 6 minecraft:torch");
		// Dark cave: sealed stone, one torch in a corner.
		command("fill 80 219 -14 110 228 14 minecraft:stone");
		command("fill 82 220 -12 108 223 12 minecraft:air");
		command("fill 90 220 -4 94 221 -2 minecraft:stone");
		command("setblock 84 220 10 minecraft:torch");
		command("time set 6000"); command("weather clear");
		ArcticClient.config().dynamicFps = false;
		ArcticClient.config().lightOverlay = true;
		ArcticClient.config().lightRange = 16;
		ArcticClient.config().lightStyle = 0;
		ArcticClient.config().lightShowSafe = true;
		command("tp @p -2.5 220 20 180 28");
		MinecraftClient.getInstance().options.hudHidden = true;
		at(10, () -> LegacyWorldTest.shot("lit-squares"));
		at(12, () -> ArcticClient.config().lightStyle = 2);
		at(16, () -> LegacyWorldTest.shot("lit-numbers"));
		at(17, () -> { ArcticClient.config().lightStyle = 1; command("tp @p -2.5 220 20 180 28"); });
		at(20, () -> LegacyWorldTest.shot("lit-crosses"));
		at(21, () -> { ArcticClient.config().lightStyle = 0; command("tp @p 92.5 220 -10 22 25"); });
		at(30, () -> LegacyWorldTest.shot("cave-squares"));
		at(31, () -> ArcticClient.config().lightShowSafe = false);
		at(36, () -> LegacyWorldTest.shot("cave-no-safe"));
		at(37, () -> { ArcticClient.config().lightShowSafe = true; ArcticClient.config().lightStyle = 2; });
		at(42, () -> LegacyWorldTest.shot("cave-numbers"));
		// Numbers read from any side: the lit area seen from the east (looking west).
		at(43, () -> command("tp @p 12.5 220 20 90 28"));
		at(47, () -> LegacyWorldTest.shot("lit-numbers-east"));
		// A torch placed in the dark cave flips the markers around it to green within a moment.
		at(48, () -> { ArcticClient.config().lightStyle = 0; command("tp @p 92.5 220 -10 22 25"); });
		at(53, () -> LegacyWorldTest.shot("cave-before-torch"));
		at(54, () -> command("setblock 88 220 2 minecraft:torch"));
		at(55, () -> LegacyWorldTest.shot("cave-torch-1s"));
		at(57, () -> { ArcticClient.config().lightStyle = 0; MinecraftClient.getInstance().options.hudHidden = false;
			com.arcticlauncher.client.menu.Menus.open("lightoverlay"); });
		at(62, () -> LegacyWorldTest.shot("settings"));
		at(63, () -> { MinecraftClient.getInstance().setScreen(null); command("tp @p -2.5 220 20 180 28"); MinecraftClient.getInstance().options.hudHidden = true; });
		if (Boolean.getBoolean("arctic.lo.fps")) {
			at(74, () -> ArcticClient.config().lightOverlay = false);
			for (int i = 0; i < 12; i++) {
				at(76 + i, () -> FPS.add(-MinecraftClient.getCurrentFps()));
			}
			at(89, () -> ArcticClient.config().lightOverlay = true);
			for (int i = 0; i < 12; i++) {
				at(92 + i, () -> FPS.add(MinecraftClient.getCurrentFps()));
			}
			at(106, () -> {
				double off = 0, on = 0; int no = 0, nn = 0;
				for (int f : FPS) { if (f < 0) { off += -f; no++; } else { on += f; nn++; } }
				ArcticLegacy.LOG.info("lightoverlay-fps off={} on={} (samples {}/{}) raw={} status={} vertices={}", no == 0 ? 0 : off / no, nn == 0 ? 0 : on / nn, no, nn, FPS, ArcticClient.lightOverlay().status(), ArcticClient.lightOverlay().mesh().vertices);
			});
			at(108, () -> MinecraftClient.getInstance().scheduleStop());
		} else {
			at(66, () -> MinecraftClient.getInstance().scheduleStop());
		}
	}
}
