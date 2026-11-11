package com.arcticlauncher.mod;
import com.arcticlauncher.client.ArcticClient;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
/** Offline-only, opt-in visual regression scene: -Darctic.selftest=ctm. */
final class GlassCtmTest {
	private static final java.util.concurrent.ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-ctm-proof"); t.setDaemon(true); return t;
	});
	private GlassCtmTest() {}
	static void start() {
		TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(GlassCtmTest::scene)), 10, TimeUnit.SECONDS);
	}
	private static void run(Runnable r) { Minecraft.getInstance().execute(r); }
	private static void command(String s) { Compat.sendChat("/" + s); }
	private static void scene() {
		command("tp @s 0.5 220 -7 0 0");
		TIMER.schedule(() -> run(GlassCtmTest::buildScene), 7, TimeUnit.SECONDS);
	}
	private static void buildScene() {
		ArcticMod.LOG.info("ctm-proof: native available={}, enabled={}", GlassCtm.available(), GlassCtm.enabled());
		command("gamemode creative");
		command("fill -10 219 -10 10 219 8 minecraft:stone");
		command("fill -10 220 -10 10 230 8 minecraft:air");
		command("fill -7 220 4 7 226 4 minecraft:stone");
		command("fill -5 220 0 -3 222 0 minecraft:glass");
		command("fill -1 220 0 1 222 0 minecraft:red_stained_glass");
		command("fill 3 221 0 5 221 0 minecraft:glass_pane");
		command("time set 6000"); command("weather clear"); command("tp @s 0.5 220 -7 0 0");
		boolean hold = Boolean.getBoolean("arctic.togglekeys");
		//#if MC >= 26.2
		if (!hold && !Minecraft.getInstance().gui.hud.isHidden()) Minecraft.getInstance().gui.hud.toggle();
		//#else
		Minecraft.getInstance().options.hideGui = !hold;
		//#endif
		ArcticClient.config().connectedGlass = true;
		if (hold) {
			// Toggle-key proof: stay in the scene; log each change so the log shows what the keys did.
			final boolean[] last = {true, true};
			TIMER.scheduleAtFixedRate(() -> {
				boolean glass = ArcticClient.config().connectedGlass;
				boolean fps = false;
				for (com.arcticlauncher.client.hud.HudWidget w : ArcticClient.hud().widgets()) {
					if ("fps".equals(w.id)) fps = ArcticClient.hud().slot(w).enabled;
				}
				if (glass != last[0] || fps != last[1]) {
					last[0] = glass;
					last[1] = fps;
					ArcticMod.LOG.info("togglekeys-proof: connectedGlass={} fpsWidget={}", glass, fps);
				}
			}, 0, 100, TimeUnit.MILLISECONDS);
			ArcticMod.LOG.info("togglekeys-proof: ready");
			return;
		}
		TIMER.schedule(() -> run(() -> WorldTest.shot("ctm-on")), 8, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> ArcticClient.config().connectedGlass = false), 12, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> WorldTest.shot("ctm-off")), 20, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> Minecraft.getInstance().stop()), 25, TimeUnit.SECONDS);
	}
}
