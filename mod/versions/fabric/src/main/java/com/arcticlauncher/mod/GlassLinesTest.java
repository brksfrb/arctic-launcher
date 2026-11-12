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
		Minecraft mc = Minecraft.getInstance();
		command("gamemode creative");
		fill(-12, 219, -8, 12, 219, 12, "minecraft:stone");
		fill(-12, 220, -8, 12, 232, 12, "minecraft:air");
		fill(-9, 220, 9, 9, 226, 9, "minecraft:bookshelf");
		fill(-4, 220, 3, 0, 223, 3, "minecraft:glass");
		fill(2, 220, 3, 5, 222, 3, "minecraft:glass_pane");
		fill(-8, 220, 3, -6, 222, 3, "minecraft:light_blue_stained_glass");
		fill(-3, 220, 0, -1, 220, 1, "minecraft:glass");
		command("time set 6000"); command("weather clear"); command("gamerule advance_time false");
		com.arcticlauncher.client.ArcticClient.config().connectedGlass = true;
		hide();
		later(4, () -> lines(new int[] {0, 100, 50}, 0));
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
		Minecraft mc = Minecraft.getInstance();
		if (i == levels.length) { later(1, () -> Minecraft.getInstance().stop()); return; }
		slide(levels[i], () -> {
			Compat.setScreen(null);
			later(1, () -> hold(-0.5, 220.4, -2.2, 0, 2, System.nanoTime(), 3, () -> {
				ArcticMod.LOG.info("ctmlines level {}: stitched {} ({})", levels[i], com.arcticlauncher.client.ctm.GlassLines.stitched(), com.arcticlauncher.client.ctm.GlassLines.stitched() == levels[i] ? "ok" : "WRONG");
				WorldTest.shot("lines-" + levels[i] + "-world");
				later(0.5, () -> lines(levels, i + 1));
			}));
		});
	}
}
//#endif
