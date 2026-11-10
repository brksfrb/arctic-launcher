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
		command("tp @p -3.5 220 -4.0 0 0");
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
		command("time set 6000"); command("weather clear"); command("tp @p -3.5 220 -4.0 0 0");
		MinecraftClient.getInstance().options.hudHidden = true;
		MinecraftClient.getInstance().focused = true;
		ArcticClient.config().connectedGlass = true;
		TIMER.schedule(() -> run(() -> {
			regression();
			LegacyWorldTest.shot("ctm-connected");
		}), 8, TimeUnit.SECONDS);
		// Capture five ticks into a real held attack, before the glass block breaks.
		TIMER.schedule(() -> run(() -> command("gamemode survival")), 9, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> net.minecraft.client.option.KeyBinding.setKeyPressed(
				MinecraftClient.getInstance().options.attackKey.getCode(), true)), 10, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> LegacyWorldTest.shot("ctm-crack")), 10250, TimeUnit.MILLISECONDS);
		TIMER.schedule(() -> run(() -> net.minecraft.client.option.KeyBinding.setKeyPressed(
				MinecraftClient.getInstance().options.attackKey.getCode(), false)), 11, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> MinecraftClient.getInstance().scheduleStop()), 15, TimeUnit.SECONDS);
	}

	/** Runs with the real vanilla quad subclasses and stitched native atlas, never in releases. */
	private static void regression() {
		MinecraftClient mc = MinecraftClient.getInstance();
		net.minecraft.util.math.BlockPos pos = new net.minecraft.util.math.BlockPos(-4, 221, 0);
		net.minecraft.block.BlockState state = mc.world.getBlockState(pos);
		net.minecraft.client.render.model.BakedModel model = mc.getBlockRenderManager().getModels().getBakedModel(state);
		//#if MC >= 1.9
		java.util.List<net.minecraft.client.render.model.BakedQuad> original = model.method_12502(state, net.minecraft.util.math.Direction.NORTH, 0L);
		//#else
		java.util.List<net.minecraft.client.render.model.BakedQuad> original = model.getByDirection(net.minecraft.util.math.Direction.NORTH);
		//#endif
		net.minecraft.client.render.model.BakedQuad glass = original.get(0);
		check(((GlassQuad) (Object) glass).arctic$ctm() != null, "glass has variants");
		net.minecraft.client.render.model.BakedQuad damage = new net.minecraft.client.render.model.TexturedBakedQuad(glass, model.getParticleSprite());
		check(((GlassQuad) (Object) damage).arctic$ctm() == null, "damage constructor has no variants");
		check(LegacyGlassCtm.prepare(damage) == null, "explicit damage prepare has no variants");
		LegacyGlassCtm.beginDamage();
		try {
			LegacyGlassCtm.begin(mc.world, state, pos);
			check(LegacyGlassCtm.quads(original) == original, "damage begin stays inactive");
			LegacyGlassCtm.beginDamage();
			LegacyGlassCtm.endDamage();
			LegacyGlassCtm.begin(mc.world, state, pos);
			check(LegacyGlassCtm.quads(original) == original, "nested damage stays inactive");
		} finally { LegacyGlassCtm.endDamage(); }
		LegacyGlassCtm.begin(mc.world, state, pos);
		java.util.List<net.minecraft.client.render.model.BakedQuad> connected = LegacyGlassCtm.quads(original);
		check(connected != original && connected.get(0) != glass, "terrain selects connected tile");
		LegacyGlassCtm.end();
		check(LegacyGlassCtm.quads(original) == original, "end clears context");
		ArcticLegacy.LOG.info("ctm-break regression PASS: constructor, damage suppression, nested damage, terrain substitution");
	}
	private static void check(boolean condition, String label) {
		if (!condition) throw new AssertionError("ctm-break: " + label);
	}
}
