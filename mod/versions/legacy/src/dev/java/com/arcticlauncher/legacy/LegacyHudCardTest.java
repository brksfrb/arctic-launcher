package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.hud.HudWidget;
import com.arcticlauncher.client.menu.HudEditor;
import java.util.Map;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;

/** Dev scene (-Darctic.selftest=hudcard, -Darctic.hudcard.phase=1|2): the HUD editor's card lets a widget under it be grabbed, and can be moved. */
final class LegacyHudCardTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-hudcard-test");
		t.setDaemon(true);
		return t;
	});

	private LegacyHudCardTest() {}

	static void start() {
		later(25, () -> {
			boolean first = !"2".equals(System.getProperty("arctic.hudcard.phase"));
			if (first) {
				HudWidget w = fps();
				HudSlot slot = ArcticClient.hud().slot(w);
				slot.enabled = true;
				slot.placed = true;
				slot.ax = HudSlot.CENTER;
				slot.ay = HudSlot.CENTER;
				slot.dx = 0;
				slot.dy = 0;
				slot.scale = 2f;
				ArcticClient.config().hudCardDx = 0;
				ArcticClient.config().hudCardDy = 0;
				ArcticClient.saveConfig();
			}
			HudEditor editor = new HudEditor();
			ArcticClient.platform().openPage(editor);
			if (first) {
				phase1(editor);
			} else {
				phase2(editor);
			}
		});
	}

	private static void phase1(HudEditor editor) {
		later(2, () -> {
			LegacyWorldTest.shot("hudcard-1-before");
			log("card at " + bx(editor) + "," + by(editor) + " widget rect " + java.util.Arrays.toString(rect(editor)));
		});
		later(3, () -> {
			int[] r = rect(editor);
			HudSlot slot = ArcticClient.hud().slot(fps());
			log("widget slot before: placed=" + slot.placed + " dx=" + slot.dx + " dy=" + slot.dy);
			double x = r[0] + r[2] / 2.0;
			double y = r[1] + r[3] / 2.0;
			editor.mouseClicked(x, y, 0);
			editor.mouseDragged(x + 6, y + 6, 0);
			editor.mouseDragged(x + 12, y + 8, 0);
		});
		later(5, () -> LegacyWorldTest.shot("hudcard-2-dimmed-while-dragging"));
		later(6, () -> {
			int[] r = rect(editor);
			double x = r[0] + r[2] / 2.0;
			double y = r[1] + r[3] / 2.0;
			editor.mouseDragged(x + 230, y + 130, 0);
		});
		later(7, () -> {
			editor.mouseReleased(0, 0, 0);
			HudSlot slot = ArcticClient.hud().slot(fps());
			log("widget slot after: dx=" + slot.dx + " dy=" + slot.dy + " -> " + (slot.dx != 0 || slot.dy != 0 ? "GRAB OK" : "GRAB FAILED"));
		});
		later(8, () -> LegacyWorldTest.shot("hudcard-3-widget-moved"));
		later(9, () -> {
			int hx = bx(editor) + 150;
			int hy = by(editor) + 10;
			editor.mouseClicked(hx, hy, 0);
			editor.mouseDragged(hx - 140, hy - 20, 0);
			editor.mouseDragged(hx - 300, hy + 100, 0);
			editor.mouseReleased(hx - 300, hy + 100, 0);
			log("card moved to dx=" + ArcticClient.config().hudCardDx + " dy=" + ArcticClient.config().hudCardDy + " bx=" + bx(editor) + " by=" + by(editor));
		});
		later(10, () -> LegacyWorldTest.shot("hudcard-4-card-moved"));
		later(11, () -> {
			// The card's buttons still work where no widget is under them.
			log("DONE phase 1");
			MinecraftClient.getInstance().scheduleStop();
		});
	}

	private static void phase2(HudEditor editor) {
		later(2, () -> {
			log("after restart: card dx=" + ArcticClient.config().hudCardDx + " dy=" + ArcticClient.config().hudCardDy + " bx=" + bx(editor) + " by=" + by(editor));
			LegacyWorldTest.shot("hudcard-5-after-restart");
		});
		later(3, () -> {
			int hx = bx(editor) + 150;
			int hy = by(editor) + 10;
			editor.mouseClicked(hx, hy, 0);
			editor.mouseReleased(hx, hy, 0);
			editor.mouseClicked(hx, hy, 0);
			editor.mouseReleased(hx, hy, 0);
			log("double-click reset: dx=" + ArcticClient.config().hudCardDx + " dy=" + ArcticClient.config().hudCardDy + " bx=" + bx(editor) + " by=" + by(editor));
		});
		later(4, () -> LegacyWorldTest.shot("hudcard-6-reset"));
		later(5, () -> {
			log("DONE phase 2");
			MinecraftClient.getInstance().scheduleStop();
		});
	}

	private static HudWidget fps() {
		for (HudWidget w : ArcticClient.hud().widgets()) {
			if (w.id.equals("fps")) {
				return w;
			}
		}
		throw new IllegalStateException("no fps widget");
	}

	private static int bx(HudEditor e) {
		return field(e, "bx");
	}

	private static int by(HudEditor e) {
		return field(e, "by");
	}

	private static int field(HudEditor e, String name) {
		try {
			java.lang.reflect.Field f = HudEditor.class.getDeclaredField(name);
			f.setAccessible(true);
			return f.getInt(e);
		} catch (ReflectiveOperationException ex) {
			throw new IllegalStateException(ex);
		}
	}

	@SuppressWarnings("unchecked")
	private static int[] rect(HudEditor e) {
		try {
			java.lang.reflect.Field f = HudEditor.class.getDeclaredField("rects");
			f.setAccessible(true);
			for (Map.Entry<HudWidget, int[]> en : ((Map<HudWidget, int[]>) f.get(e)).entrySet()) {
				if (en.getKey().id.equals("fps")) {
					return en.getValue();
				}
			}
		} catch (ReflectiveOperationException ex) {
			throw new IllegalStateException(ex);
		}
		throw new IllegalStateException("no fps rect");
	}

	private static void log(String s) {
		ArcticLegacy.LOG.info("hudcard: {}", s);
	}

	private static void later(int seconds, Runnable r) {
		TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> {
			try {
				r.run();
			} catch (Throwable t) {
				ArcticLegacy.LOG.error("hudcard: failed", t);
				MinecraftClient.getInstance().scheduleStop();
			}
		}), seconds, TimeUnit.SECONDS);
	}
}
