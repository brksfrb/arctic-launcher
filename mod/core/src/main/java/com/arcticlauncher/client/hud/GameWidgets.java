package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;

/**
 * HUD widgets that read the game (armor, effects, combat): they need the
 * version's game hooks, so they're hidden where those don't exist yet.
 */
final class GameWidgets {
	private static final int ICON = 16;
	private static final int ROW = 17;
	private static final int PAD = 4;

	private GameWidgets() {}

	private static Platform platform() {
		return ArcticClient.platform();
	}

	static List<HudWidget> all() {
		List<HudWidget> all = new ArrayList<HudWidget>();
		all.add(new Armor());
		all.add(new Effects());
		all.add(new Held());
		all.add(new Target());
		all.add(new GameText("reach", "Reach", "Distance of your last hit") {
			@Override
			protected String value(boolean preview) {
				double reach = ArcticClient.features().combat().reach();
				if (reach < 0) {
					return preview ? "3.00 m" : "--";
				}
				return String.format(Locale.ROOT, "%.2f m", reach);
			}
		});
		all.add(new GameText("combo", "Combo", "Hits in a row without getting hit") {
			@Override
			protected String value(boolean preview) {
				return String.valueOf(ArcticClient.features().combat().combo());
			}
		});
		all.add(new GameText("movement", "Move", "Shows toggled sprint or sneak") {
			@Override
			protected String value(boolean preview) {
				String m = ArcticClient.features().movement();
				return m == null ? "Walking" : m;
			}
		});
		return all;
	}

	/** A text widget that needs game hooks. */
	private abstract static class GameText extends TextWidget {
		GameText(String id, String label, String description) {
			super(id, label, description, false);
		}

		@Override
		public boolean needsGame() {
			return true;
		}
	}

	/** Worn armor and the held item, with durability. */
	private static final class Armor extends HudWidget {
		private static final String SAMPLE = "0000";

		Armor() {
			super("armor", "Armor", "Worn armor and durability", false, Column.RIGHT);
		}

		@Override
		public boolean needsGame() {
			return true;
		}

		private static List<Object[]> rows(boolean preview) {
			List<Object[]> rows = platform().armor();
			if (rows.isEmpty() && preview) {
				// Sample durabilities, so the editor shows what it will look like.
				return Arrays.asList(new Object[] {null, "363"}, new Object[] {null, "528"},
						new Object[] {null, "495"}, new Object[] {null, "429"});
			}
			return rows;
		}

		@Override
		public int width(Gfx g) {
			return PAD * 2 + ICON + 3 + g.textWidth(SAMPLE);
		}

		@Override
		public int height() {
			return PAD + Math.max(1, rows(true).size()) * ROW;
		}

		@Override
		public void render(Gfx g, Style s, boolean preview) {
			List<Object[]> rows = rows(preview);
			if (rows.isEmpty()) {
				return;
			}
			Draw.round(g, 0, 0, width(g), PAD + rows.size() * ROW, 2, s.hud);
			int y = PAD / 2;
			for (Object[] row : rows) {
				if (row[0] != null) {
					g.item(row[0], PAD, y);
				} else {
					placeholderIcon(g, PAD, y, s);
				}
				if (row[1] != null) {
					g.text((String) row[1], PAD + ICON + 3, y + 4, s.text, false);
				}
				y += ROW;
			}
		}
	}

	/** A stand-in item icon for the editor: a small tinted square. */
	private static void placeholderIcon(Gfx g, int x, int y, Style s) {
		Draw.round(g, x + 2, y + 2, x + ICON - 2, y + ICON - 2, 2, Draw.alpha(s.accent, 0.45f));
		Draw.outline(g, x + 2, y + 2, x + ICON - 2, y + ICON - 2, 2, Draw.alpha(s.accent, 0.8f));
	}

	/** Active potion effects with time left. */
	private static final class Effects extends HudWidget {
		private static final int LINE = 11;

		Effects() {
			super("effects", "Effects", "Active effects and time left", false, Column.RIGHT);
		}

		@Override
		public boolean needsGame() {
			return true;
		}

		private static List<Object[]> rows(boolean preview) {
			List<Object[]> rows = platform().effects();
			if (rows.isEmpty() && preview) {
				List<Object[]> sample = new ArrayList<Object[]>();
				sample.add(new Object[] {"Speed II", "1:30", 0xFF7CAFC6});
				sample.add(new Object[] {"Night Vision", "4:52", 0xFF1F1FA1});
				return sample;
			}
			return rows;
		}

		@Override
		public int width(Gfx g) {
			int w = g.textWidth("Night Vision II  0:00");
			for (Object[] r : rows(true)) {
				w = Math.max(w, g.textWidth(r[0] + "  " + r[1]));
			}
			return PAD * 2 + 5 + w;
		}

		@Override
		public int height() {
			return PAD + Math.max(1, rows(true).size()) * LINE;
		}

		@Override
		public void render(Gfx g, Style s, boolean preview) {
			List<Object[]> rows = rows(preview);
			if (rows.isEmpty()) {
				return;
			}
			int w = width(g);
			Draw.round(g, 0, 0, w, PAD + rows.size() * LINE, 2, s.hud);
			int y = PAD / 2 + 1;
			for (Object[] r : rows) {
				g.fill(PAD, y, PAD + 2, y + 8, 0xFF000000 | (Integer) r[2]);
				g.text((String) r[0], PAD + 5, y, s.text, false);
				String time = (String) r[1];
				g.text(time, w - PAD - g.textWidth(time), y, s.muted, false);
				y += LINE;
			}
		}
	}

	/** The held item and how many of it you carry (arrows, blocks…). */
	private static final class Held extends HudWidget {
		Held() {
			super("held", "Held item", "How many of the held item you carry", false, Column.RIGHT);
		}

		@Override
		public boolean needsGame() {
			return true;
		}

		@Override
		public int width(Gfx g) {
			return PAD * 2 + ICON + 3 + g.textWidth("0000");
		}

		@Override
		public int height() {
			return ICON + PAD;
		}

		@Override
		public void render(Gfx g, Style s, boolean preview) {
			Object[] held = platform().heldItem();
			if (held == null && !preview) {
				return;
			}
			Draw.round(g, 0, 0, width(g), height(), 2, s.hud);
			if (held == null) {
				placeholderIcon(g, PAD, PAD / 2, s);
				g.text("64", PAD + ICON + 3, PAD / 2 + 4, s.text, false);
				return;
			}
			g.item(held[0], PAD, PAD / 2);
			g.text(String.valueOf(held[1]), PAD + ICON + 3, PAD / 2 + 4, s.text, false);
		}
	}

	/** Who you're fighting: name and a health bar. */
	private static final class Target extends HudWidget {
		private static final int W = 110;
		private static final int H = 26;

		Target() {
			super("target", "Target", "Health of what you're aiming at or hit", false, Column.LEFT);
		}

		@Override
		public boolean needsGame() {
			return true;
		}

		@Override
		public int width(Gfx g) {
			return W;
		}

		@Override
		public int height() {
			return H;
		}

		@Override
		public void render(Gfx g, Style s, boolean preview) {
			Object[] t = platform().target();
			if (t == null && preview) {
				t = new Object[] {"Zombie", 14f, 20f};
			}
			if (t == null) {
				return;
			}
			float health = (Float) t[1];
			float max = Math.max(1f, (Float) t[2]);
			Draw.round(g, 0, 0, W, H, 2, s.hud);
			g.text(Draw.fit(g, (String) t[0], W - 50), PAD, PAD, s.text, false);
			String hp = String.format(Locale.ROOT, "%.1f / %.0f", health, max);
			g.text(hp, W - PAD - g.textWidth(hp), PAD, s.muted, false);
			int barY = H - PAD - 5;
			Draw.round(g, PAD, barY, W - PAD, barY + 5, 1, Draw.alpha(s.muted, 0.35f));
			int filled = Math.round((W - PAD * 2) * Math.max(0f, Math.min(1f, health / max)));
			float ratio = health / max;
			int color = ratio > 0.5f ? 0xFF4ADE80 : ratio > 0.25f ? 0xFFFACC15 : 0xFFF87171;
			if (filled > 0) {
				Draw.round(g, PAD, barY, PAD + filled, barY + 5, 1, color);
			}
		}
	}
}
