package com.arcticlauncher.mod;
import com.arcticlauncher.client.ArcticClient;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
/** Offline-only, opt-in proof scene: -Darctic.selftest=lightoverlay (dark cave, lit area, settings page, FPS off/on). */
final class LightOverlayTest {
	private static final java.util.concurrent.ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-lightoverlay-proof"); t.setDaemon(true); return t;
	});
	private static final java.util.List<Integer> FPS = new java.util.concurrent.CopyOnWriteArrayList<Integer>();
	private LightOverlayTest() {}
	static void start() {
		TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(LightOverlayTest::scene)), 10, TimeUnit.SECONDS);
	}
	private static void run(Runnable r) { Minecraft.getInstance().execute(r); }
	private static void command(String s) { Compat.sendChat("/" + s); }
	private static void at(int seconds, Runnable r) { TIMER.schedule(() -> run(r), seconds, TimeUnit.SECONDS); }
	private static void hud(boolean hidden) {
		//#if MC >= 26.2
		if (Minecraft.getInstance().gui.hud.isHidden() != hidden) Minecraft.getInstance().gui.hud.toggle();
		//#else
		Minecraft.getInstance().options.hideGui = hidden;
		//#endif
	}
	private static void scene() {
		command("tp @s 0.5 220 -7 0 0");
		TIMER.schedule(() -> run(LightOverlayTest::build), 7, TimeUnit.SECONDS);
	}
	private static void build() {
		command("gamemode creative");
		command("fill -14 219 -14 14 219 44 minecraft:stone");
		command("fill -14 220 -14 14 232 44 minecraft:air");
		command("fill -14 224 4 14 224 44 minecraft:stone");
		command("setblock 8 220 6 minecraft:torch");
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
		command("tp @s -2.5 220 20 180 28");
		hud(true);
		at(10, () -> WorldTest.shot("lit-squares"));
		at(12, () -> ArcticClient.config().lightStyle = 2);
		at(16, () -> WorldTest.shot("lit-numbers"));
		at(17, () -> { ArcticClient.config().lightStyle = 1; command("tp @s -2.5 220 20 180 28"); });
		at(20, () -> WorldTest.shot("lit-crosses"));
		at(21, () -> { ArcticClient.config().lightStyle = 0; command("tp @s 92.5 220 -10 22 25"); });
		at(30, () -> WorldTest.shot("cave-squares"));
		at(31, () -> ArcticClient.config().lightShowSafe = false);
		at(36, () -> WorldTest.shot("cave-no-safe"));
		at(37, () -> { ArcticClient.config().lightShowSafe = true; ArcticClient.config().lightStyle = 2; });
		at(42, () -> WorldTest.shot("cave-numbers"));
		at(43, () -> { ArcticClient.config().lightStyle = 0; hud(false); com.arcticlauncher.client.menu.Menus.open("lightoverlay"); });
		at(48, () -> WorldTest.shot("settings"));
		at(49, () -> { Compat.setScreen(null); command("tp @s -2.5 220 20 180 28"); hud(true); });
		if (Boolean.getBoolean("arctic.lo.fps")) {
			at(60, () -> ArcticClient.config().lightOverlay = false);
			for (int i = 0; i < 12; i++) {
				at(62 + i, () -> FPS.add(-Minecraft.getInstance().getFps()));
			}
			at(75, () -> ArcticClient.config().lightOverlay = true);
			for (int i = 0; i < 12; i++) {
				at(78 + i, () -> FPS.add(Minecraft.getInstance().getFps()));
			}
			at(92, () -> {
				double off = 0, on = 0; int no = 0, nn = 0;
				for (int f : FPS) { if (f < 0) { off += -f; no++; } else { on += f; nn++; } }
				ArcticMod.LOG.info("lightoverlay-fps off={} on={} (samples {}/{}) raw={} status={} vertices={}", no == 0 ? 0 : off / no, nn == 0 ? 0 : on / nn, no, nn, FPS, ArcticClient.lightOverlay().status(), ArcticClient.lightOverlay().mesh().vertices);
			});
			at(94, () -> Minecraft.getInstance().stop());
		} else {
			at(55, () -> Minecraft.getInstance().stop());
		}
	}
}
