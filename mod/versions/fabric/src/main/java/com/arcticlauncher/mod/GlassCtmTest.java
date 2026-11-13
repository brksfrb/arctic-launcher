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
		command("tp @s -4.5 220 -4.0 0 0");
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
		command("time set 6000"); command("weather clear"); command("tp @s -4.5 220 -4.0 0 0");
		boolean hold = Boolean.getBoolean("arctic.togglekeys");
		if (hold) command("tp @s 0.5 220 -7 0 0");
		//#if MC >= 26.2
		if (!hold && !Minecraft.getInstance().gui.hud.isHidden()) Minecraft.getInstance().gui.hud.toggle();
		//#else
		Minecraft.getInstance().options.hideGui = !hold;
		//#endif
		//#if MC < 1.15
		Minecraft.getInstance().mouseHandler.grabMouse();
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
		TIMER.schedule(() -> run(() -> {
			//#if MC < 1.15
			regression();
			//#endif
			WorldTest.shot("ctm-connected");
		}), 8, TimeUnit.SECONDS);
		//#if MC < 1.15
		// Capture five ticks into a real held attack, before the glass block breaks.
		TIMER.schedule(() -> run(() -> command("gamemode survival")), 9, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> attack(true)), 10, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> WorldTest.shot("ctm-crack")), 10250, TimeUnit.MILLISECONDS);
		TIMER.schedule(() -> run(() -> attack(false)), 11, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> command("tp @s 4.5 220 -4.0 0 0")), 12, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> attack(true)), 13, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> WorldTest.shot("ctm-crack-pane")), 13150, TimeUnit.MILLISECONDS);
		TIMER.schedule(() -> run(() -> attack(false)), 14, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> Minecraft.getInstance().stop()), 17, TimeUnit.SECONDS);
		//#else
		// Real held attack on the block, then on the pane; capture mid-break.
		TIMER.schedule(() -> run(() -> command("gamemode survival")), 9, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> attack(true)), 10, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> WorldTest.shot("ctm-crack")), 10250, TimeUnit.MILLISECONDS);
		TIMER.schedule(() -> run(() -> attack(false)), 11, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> command("tp @s 4.5 220 -4.0 0 0")), 12, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> attack(true)), 13, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> WorldTest.shot("ctm-crack-pane")), 13150, TimeUnit.MILLISECONDS);
		TIMER.schedule(() -> run(() -> attack(false)), 14, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> Minecraft.getInstance().stop()), 17, TimeUnit.SECONDS);
		//#endif
	}
	private static void attack(boolean down) {
		Minecraft mc = Minecraft.getInstance();
		//#if MC >= 1.15
		mc.options.keyAttack.setDown(down);
		//#else
		if (down) mc.mouseHandler.grabMouse();
		// Use the vanilla left mouse binding; 1.13's keyAttack field is misnamed in its mappings.
		net.minecraft.client.KeyMapping.set(com.mojang.blaze3d.platform.InputConstants.Type.MOUSE.getOrCreate(0), down);
		ArcticMod.LOG.info("ctm-break attack={}, focused={}, grabbed={}", down, mc.isWindowActive(), mc.mouseHandler.isMouseGrabbed());
		//#endif
	}
	//#if MC < 1.15
	private static void regression() {
		Minecraft mc = Minecraft.getInstance();
		net.minecraft.core.BlockPos pos = new net.minecraft.core.BlockPos(-4, 221, 0);
		net.minecraft.world.level.block.state.BlockState state = mc.level.getBlockState(pos);
		net.minecraft.client.resources.model.BakedModel model = mc.getBlockRenderer().getBlockModel(state);
		java.util.List<net.minecraft.client.renderer.block.model.BakedQuad> original = model.getQuads(state, net.minecraft.core.Direction.NORTH, new java.util.Random(0));
		net.minecraft.client.renderer.block.model.BakedQuad glass = original.get(0);
		check(((GlassQuad) (Object) glass).arctic$ctm() != null, "glass has variants");
		net.minecraft.client.renderer.block.model.BakedQuad damage = new net.minecraft.client.renderer.block.model.BreakingQuad(glass, model.getParticleIcon());
		check(((GlassQuad) (Object) damage).arctic$ctm() == null, "damage constructor has no variants");
		check(GlassCtm.prepare(damage) == null, "explicit damage prepare has no variants");
		GlassCtm.beginDamage();
		try {
			GlassCtm.begin(mc.level, state, pos);
			check(GlassCtm.quads(original) == original, "damage begin stays inactive");
		} finally { GlassCtm.endDamage(); }
		GlassCtm.begin(mc.level, state, pos);
		java.util.List<net.minecraft.client.renderer.block.model.BakedQuad> connected = GlassCtm.quads(original);
		check(connected != original && connected.get(0) != glass, "terrain selects connected tile");
		GlassCtm.end();
		ArcticMod.LOG.info("ctm-break regression PASS: BreakingQuad constructor, damage suppression, terrain substitution");
	}
	private static void check(boolean condition, String label) {
		if (!condition) throw new AssertionError("ctm-break: " + label);
	}
	//#endif

}
