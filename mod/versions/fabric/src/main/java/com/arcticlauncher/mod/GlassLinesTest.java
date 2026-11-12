//#if MC >= 1.21.11
package com.arcticlauncher.mod;

import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.world.phys.Vec3;

/** -Darctic.selftest=ctmlines: the Connected Glass "Interior lines" slider, moved on the real settings page. */
final class GlassLinesTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> { Thread t = new Thread(r, "ctmlines"); t.setDaemon(true); return t; });
	private GlassLinesTest() {}
	private static void hide() {
		Minecraft mc = Minecraft.getInstance();
		//#if MC >= 26.2
		if (!mc.gui.hud.isHidden()) mc.gui.hud.toggle();
		//#else
		mc.options.hideGui = true;
		//#endif
	}
	private static void later(double seconds, Runnable r) { TIMER.schedule(() -> Minecraft.getInstance().execute(r), (long) (seconds * 1000), TimeUnit.MILLISECONDS); }
	private static void command(String c) { Compat.sendChat("/" + c); }
	private static void fill(int x1, int y1, int z1, int x2, int y2, int z2, String block) { command("fill " + x1 + " " + y1 + " " + z1 + " " + x2 + " " + y2 + " " + z2 + " " + block); }
	private static void hold(double x, double y, double z, float yaw, float pitch, long start, double seconds, Runnable then) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.player == null) return;
		mc.player.getAbilities().flying = true;
		mc.player.setPos(x, y, z);
		mc.player.setDeltaMovement(Vec3.ZERO);
		mc.player.setYRot(yaw); mc.player.setXRot(pitch);
		if ((System.nanoTime() - start) / 1e9 >= seconds) { then.run(); return; }
		later(0.02, () -> hold(x, y, z, yaw, pitch, start, seconds, then));
	}

	/**
	 * -Darctic.selftest=ctmlines: the "Lines between blocks" slider, changed while in the world. Each level is
	 * set as the slider sets it; the picture must follow without a restart. Clear glass, its panes, and stained
	 * glass beside them (which the slider leaves alone); then the settings page with its preview.
	 */
	static void startLines() { later(10, () -> WorldTest.createTestWorld(() -> { command("tp @p 0.5 220 -2.5 0 0"); later(8, GlassLinesTest::buildLines); })); }
	private static void buildLines() {
		command("gamemode creative");
		fill(-14, 219, -8, 20, 219, 12, "minecraft:stone");
		fill(-14, 220, -8, 20, 232, 12, "minecraft:air");
		fill(-14, 220, 9, 20, 226, 9, "minecraft:bookshelf");
		fill(-9, 220, 3, -7, 222, 3, "minecraft:glass");
		fill(-4, 220, 3, -2, 222, 3, "minecraft:glass_pane");
		fill(3, 220, 3, 7, 220, 3, "minecraft:glass_pane");
		fill(9, 220, 3, 9, 220, 3, "minecraft:glass_pane");
		fill(11, 220, 3, 13, 222, 3, "minecraft:light_blue_stained_glass");
		fill(16, 220, 3, 18, 222, 3, "minecraft:light_blue_stained_glass_pane");
		command("time set 6000"); command("weather clear"); command("gamerule advance_time false");
		if ("false".equals(System.getProperty("arctic.glass.ctm"))) com.arcticlauncher.client.ArcticClient.config().connectedGlass = false;
		else com.arcticlauncher.client.ArcticClient.config().connectedGlass = true;
		hide();
		later(4, () -> lines(new int[] {Integer.getInteger("arctic.glass.first", 0), 50, 100, 0}, 0));
	}
	private static void status(int level) {
		int n = -1;
		try {
			java.lang.reflect.Field f = GlassCtm.class.getDeclaredField("sprites");
			f.setAccessible(true);
			n = ((java.util.Map<?, ?>) f.get(null)).size();
		} catch (ReflectiveOperationException e) { n = -2; }
		ArcticMod.LOG.info("ctmlines status level={} slider={} stitched={} connectedGlass={} available={} enabled={} ctmSpritesBound={} continuity={}", level,
				com.arcticlauncher.client.ArcticClient.config().clearGlassLines, com.arcticlauncher.client.ctm.GlassLines.stitched(),
				com.arcticlauncher.client.ArcticClient.config().connectedGlass, GlassCtm.available(), GlassCtm.enabled(), n,
				net.fabricmc.loader.api.FabricLoader.getInstance().isModLoaded("continuity"));
	}
	/** Moves the real slider on the real settings page (click, release), as a hand does, then lets the reload finish with the page still open. */
	private static void slide(int level, Runnable then) {
		com.arcticlauncher.client.ui.Page page = com.arcticlauncher.client.menu.Menus.page("connected-glass");
		com.arcticlauncher.client.ArcticClient.platform().openPage(page);
		later(2, () -> {
			try {
				java.lang.reflect.Field f = com.arcticlauncher.client.ui.Page.class.getDeclaredField("widgets");
				f.setAccessible(true);
				for (Object o : (java.util.List<?>) f.get(page)) {
					com.arcticlauncher.client.ui.Widget w = (com.arcticlauncher.client.ui.Widget) o;
					if (!w.getClass().getSimpleName().equals("SliderBar")) continue;
					double mx = w.x + 3 + level / 100.0 * (w.w - 6), my = w.y + w.h / 2.0;
					page.mouseClicked(mx, my, com.arcticlauncher.client.Keys.MOUSE_LEFT);
					page.mouseReleased(mx, my, com.arcticlauncher.client.Keys.MOUSE_LEFT);
					break;
				}
			} catch (ReflectiveOperationException e) { throw new IllegalStateException(e); }
			ArcticMod.LOG.info("ctmlines slider set to " + com.arcticlauncher.client.ArcticClient.config().clearGlassLines + " by the widget (wanted " + level + ")");
			later(RELOAD_WAIT, then);
		});
	}
	private static final double RELOAD_WAIT = 30;
	private static void lines(int[] levels, int i) {
		if (i == levels.length) { later(1, () -> Minecraft.getInstance().stop()); return; }
		slide(levels[i], () -> {
			Compat.setScreen(null);
			status(levels[i]);
			String tag = (i == 3 ? "back0-" : "") + levels[i];
			later(1, () -> hold(-5.5, 221.5, -1.5, 0, 0, System.nanoTime(), 3, () -> {
				WorldTest.shot("lines-" + tag + "-A");
				later(0.5, () -> hold(6, 220.8, -0.5, 0, 0, System.nanoTime(), 2, () -> {
					WorldTest.shot("lines-" + tag + "-B");
					later(0.5, () -> hold(14.5, 221.5, -1.5, 0, 0, System.nanoTime(), 2, () -> {
						WorldTest.shot("lines-" + tag + "-C");
						later(0.5, () -> lines(levels, i + 1));
					}));
				}));
			}));
		});
	}
}
//#endif
