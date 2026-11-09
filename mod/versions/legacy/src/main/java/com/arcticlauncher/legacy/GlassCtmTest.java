package com.arcticlauncher.legacy;
import com.arcticlauncher.client.ArcticClient;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;
/** Offline-only, opt-in visual regression scene: -Darctic.selftest=ctm. */
final class GlassCtmTest {
	private static final java.util.concurrent.ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-ctm-proof"); t.setDaemon(true); return t;
	});
	private GlassCtmTest() {}
	static void start() {
		TIMER.schedule(() -> run(() -> LegacyWorldTest.createTestWorld(GlassCtmTest::scene)), 10, TimeUnit.SECONDS);
	}
	private static void run(Runnable r) { LegacyHooks.onNextFrame(r); }
	private static void command(String s) { LegacyWorldTest.command(s); }
	private static void scene() {
		command("tp @p 0.5 220 -7 0 0");
		TIMER.schedule(() -> run(GlassCtmTest::buildScene), 7, TimeUnit.SECONDS);
	}
	private static void buildScene() {
		ArcticLegacy.LOG.info("ctm-proof: native available={}, enabled={}", LegacyGlassCtm.available(), LegacyGlassCtm.enabled());
		command("gamemode creative");
		command("fill -10 219 -10 10 219 8 minecraft:stone");
		command("fill -10 220 -10 10 230 8 minecraft:air");
		command("fill -7 220 4 7 226 4 minecraft:stone");
		command("fill -5 220 0 -3 222 0 minecraft:glass");
		command("fill -1 220 0 1 222 0 minecraft:stained_glass 14");
		command("fill 3 221 0 5 221 0 minecraft:glass_pane");
		command("time set 6000"); command("weather clear"); command("tp @p 0.5 220 -7 0 0");
		MinecraftClient.getInstance().options.hudHidden = true;
		ArcticClient.config().connectedGlass = true;
		TIMER.schedule(() -> run(() -> LegacyWorldTest.shot("ctm-on")), 8, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> ArcticClient.config().connectedGlass = false), 12, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> LegacyWorldTest.shot("ctm-off")), 20, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> MinecraftClient.getInstance().scheduleStop()), 25, TimeUnit.SECONDS);
	}
}
