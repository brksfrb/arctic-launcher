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
		command("tp @p -4.5 220 -4.0 0 0");
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
		command("time set 6000"); command("weather clear"); command("tp @p -4.5 220 -4.0 0 0");
		boolean hold = Boolean.getBoolean("arctic.togglekeys");
		if (hold) command("tp @p 0.5 220 -7 0 0");
		MinecraftClient.getInstance().options.hudHidden = !hold;
		MinecraftClient.getInstance().focused = true;
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
					ArcticLegacy.LOG.info("togglekeys-proof: connectedGlass={} fpsWidget={}", glass, fps);
				}
			}, 0, 100, TimeUnit.MILLISECONDS);
			ArcticLegacy.LOG.info("togglekeys-proof: ready");
			return;
		}
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
		// Then the glass pane.
		TIMER.schedule(() -> run(() -> command("tp @p 4.5 220 -4.0 0 0")), 12, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> net.minecraft.client.option.KeyBinding.setKeyPressed(
				MinecraftClient.getInstance().options.attackKey.getCode(), true)), 13, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> LegacyWorldTest.shot("ctm-crack-pane")), 13150, TimeUnit.MILLISECONDS);
		TIMER.schedule(() -> run(() -> net.minecraft.client.option.KeyBinding.setKeyPressed(
				MinecraftClient.getInstance().options.attackKey.getCode(), false)), 14, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> MinecraftClient.getInstance().scheduleStop()), 17, TimeUnit.SECONDS);
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
		//#if MC < 1.9
		// Like Forge's UnpackedBakedQuad: its vertex storage is assigned after super().
		net.minecraft.client.render.model.BakedQuad unpacked = new net.minecraft.client.render.model.BakedQuad(
				glass.getVertexData(), glass.getColorIndex(), glass.getFace()) {
			private final int[] unpackedData = glass.getVertexData();
			@Override public int[] getVertexData() {
				if (unpackedData == null) throw new NullPointerException("subclass vertex data not initialized");
				return unpackedData;
			}
		};
		check(((GlassQuad) (Object) unpacked).arctic$ctm() == null, "unpacked constructor has no variants");
		check(LegacyGlassCtm.prepare(unpacked) == null, "explicit unpacked prepare has no variants");
		//#endif
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
