package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.hud.HudWidget;
import com.arcticlauncher.client.looks.Look;
import com.arcticlauncher.client.looks.Looks;
import com.arcticlauncher.client.looks.Preset;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.KeyButton;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.Toggle;
import java.util.List;

/** The Arctic menu (Right Shift in game): HUD, looks and style. */
public final class ArcticMenu extends Page {
	private enum Tab {
		HUD("HUD", "Choose what shows on screen."),
		FEATURES("Features", "Hold a key to zoom or look around freely."),
		LOOKS("Looks", "Every Arctic player sees your cape."),
		STYLE("Style", "How Minecraft's menus look.");

		final String title;
		final String hint;

		Tab(String title, String hint) {
			this.title = title;
			this.hint = hint;
		}
	}

	private static final int SIDEBAR = 92;
	private static final int PAD = 10;
	private static final int ROW = 24;
	private static final int HUD_ROW = 18;
	private static final int KEY_W = 76;
	private static final int MAX_NEARBY = 3;
	/** Remembered while the game runs. */
	private static Tab tab = Tab.HUD;

	private int px;
	private int py;
	private int pw;
	private int ph;
	/** What the Looks tab showed at its last build (rebuild only on change). */
	private String shownLooks = "";
	/** Key buttons on the Features tab, which take the next key press. */
	private final java.util.List<KeyButton> keyButtons = new java.util.ArrayList<KeyButton>();
	/** The cape list was asked for once since this menu opened. */
	private boolean requestedCapes;

	/** Open on a tab next time ("hud", "looks" or "style"). */
	public static void showTab(String name) {
		for (Tab t : Tab.values()) {
			if (t.name().equalsIgnoreCase(name)) {
				tab = t;
			}
		}
	}

	@Override
	public boolean pausesGame() {
		return false;
	}

	private int contentX() {
		return px + SIDEBAR + PAD;
	}

	private int contentW() {
		return pw - SIDEBAR - PAD * 2;
	}

	private int contentTop() {
		return py + 38;
	}

	@Override
	protected void build() {
		pw = Math.min(440, width - 20);
		ph = Math.min(280, height - 20);
		px = (width - pw) / 2;
		py = (height - ph) / 2;
		int y = py + 34;
		if (tab == Tab.FEATURES && !ArcticClient.platform().hasFeatures()) {
			tab = Tab.HUD;
		}
		for (final Tab t : Tab.values()) {
			if (t == Tab.FEATURES && !ArcticClient.platform().hasFeatures()) {
				continue;
			}
			add(new Button(t.title, new Runnable() {
				@Override
				public void run() {
					tab = t;
					rebuild();
				}
			}).selected(t == tab)).bounds(px + 8, y, SIDEBAR - 16, 20);
			y += 24;
		}
		add(new Button("Done", new Runnable() {
			@Override
			public void run() {
				close();
			}
		})).bounds(px + 8, py + ph - 28, SIDEBAR - 16, 20);
		keyButtons.clear();
		if (tab == Tab.HUD) {
			buildHud();
		} else if (tab == Tab.FEATURES) {
			buildFeatures();
		} else if (tab == Tab.LOOKS) {
			buildLooks();
		} else {
			buildStyle();
		}
	}

	// ---- HUD -------------------------------------------------------------------

	private void buildHud() {
		add(new Button("Edit layout", new Runnable() {
			@Override
			public void run() {
				ArcticClient.platform().openPage(new HudEditor());
			}
		}).primary()).bounds(contentX() + contentW() - 80, py + 10, 80, 18);
		// Two columns of switches; the editor places what's on.
		int colW = (contentW() - 6) / 2;
		int i = 0;
		for (HudWidget w : ArcticClient.hud().widgets()) {
			final HudSlot slot = ArcticClient.hud().slot(w);
			int x = contentX() + (i % 2) * (colW + 6);
			int y = contentTop() + (i / 2) * (HUD_ROW + 2);
			add(new Toggle(w.name, null, new Toggle.Binding() {
				@Override
				public boolean get() {
					return slot.enabled;
				}

				@Override
				public void set(boolean on) {
					slot.enabled = on;
					ArcticClient.saveConfig();
				}
			})).bounds(x, y, colW, HUD_ROW);
			i++;
		}
	}

	// ---- Features --------------------------------------------------------------

	private void buildFeatures() {
		final ClientConfig c = ArcticClient.config();
		int y = contentTop();
		feature(y, "Zoom", "Hold to zoom in, like a spyglass", new Toggle.Binding() {
			@Override
			public boolean get() {
				return c.zoomEnabled;
			}

			@Override
			public void set(boolean on) {
				c.zoomEnabled = on;
				ArcticClient.saveConfig();
			}
		}, new KeyButton.Binding() {
			@Override
			public String get() {
				return c.zoomKey;
			}

			@Override
			public void set(String key) {
				c.zoomKey = key;
			}
		});
		y += ROW + 4;
		feature(y, "Freelook", "Hold to look around without turning", new Toggle.Binding() {
			@Override
			public boolean get() {
				return c.freelookEnabled;
			}

			@Override
			public void set(boolean on) {
				c.freelookEnabled = on;
				ArcticClient.saveConfig();
			}
		}, new KeyButton.Binding() {
			@Override
			public String get() {
				return c.freelookKey;
			}

			@Override
			public void set(String key) {
				c.freelookKey = key;
			}
		});
		y += ROW + 4;
		feature(y, "Fullbright", "See in the dark; the key switches it", new Toggle.Binding() {
			@Override
			public boolean get() {
				return c.fullbright;
			}

			@Override
			public void set(boolean on) {
				c.fullbright = on;
				ArcticClient.saveConfig();
			}
		}, new KeyButton.Binding() {
			@Override
			public String get() {
				return c.fullbrightKey;
			}

			@Override
			public void set(String key) {
				c.fullbrightKey = key;
			}
		});
	}

	/** A switch with a key button beside it. */
	private void feature(int y, String name, String hint, Toggle.Binding on, KeyButton.Binding key) {
		add(new Toggle(name, hint, on)).bounds(contentX(), y, contentW() - KEY_W - 6, ROW);
		KeyButton button = add(new KeyButton(key));
		button.bounds(contentX() + contentW() - KEY_W, y + 3, KEY_W, ROW - 6);
		keyButtons.add(button);
	}

	@Override
	public boolean keyPressed(int key, int nativeKey) {
		for (KeyButton b : keyButtons) {
			if (b.keyPressed(key, nativeKey)) {
				return true;
			}
		}
		return super.keyPressed(key, nativeKey);
	}

	// ---- Looks -----------------------------------------------------------------

	private void buildLooks() {
		Looks looks = ArcticClient.looks();
		if (!requestedCapes && looks.presets().isEmpty() && !looks.busy()) {
			requestedCapes = true;
			looks.open();
		}
		shownLooks = looksState();
		int bottom = capes(looks);
		if (looks.presets().isEmpty() && !looks.busy()) {
			add(new Button("Try again", new Runnable() {
				@Override
				public void run() {
					ArcticClient.looks().open();
				}
			})).bounds(contentX() + contentW() - 80, py + 10, 80, 18);
		}
		final ClientConfig config = ArcticClient.config();
		add(new Toggle("Show Arctic looks", "Other players' Arctic skins and capes", new Toggle.Binding() {
			@Override
			public boolean get() {
				return config.showCosmetics;
			}

			@Override
			public void set(boolean on) {
				config.showCosmetics = on;
				ArcticClient.saveConfig();
			}
		})).bounds(contentX(), bottom + 8, contentW(), ROW);
		nearby(bottom + 8 + ROW + 4);
	}

	/** The cape grid; returns its bottom edge. */
	private int capes(Looks looks) {
		List<Preset> presets = looks.presets();
		Look mine = looks.myLook();
		String worn = mine == null ? null : mine.cape;
		int count = presets.size() + 1;
		int cols = Math.max(1, Math.min(count, contentW() / 50));
		int tileW = (contentW() - (cols - 1) * 4) / cols;
		int tileH = 58;
		boolean canWear = looks.signedIn() && !looks.busy();
		for (int i = 0; i < count; i++) {
			Preset p = i == 0 ? null : presets.get(i - 1);
			String texture = p == null ? null : p.texture;
			boolean isWorn = texture == null ? worn == null : texture.equals(worn);
			CapeTile tile = new CapeTile(p == null ? null : p.id, p == null ? "No cape" : p.name, texture, isWorn);
			tile.enabled = canWear;
			int x = contentX() + (i % cols) * (tileW + 4);
			int y = contentTop() + (i / cols) * (tileH + 4);
			add(tile).bounds(x, y, tileW, tileH);
		}
		int rows = (count + cols - 1) / cols;
		return contentTop() + rows * (tileH + 4) - 4;
	}

	/** Hide or show a nearby player's look. */
	private void nearby(int y) {
		final ClientConfig config = ArcticClient.config();
		int shown = 0;
		for (Object[] player : ArcticClient.platform().otherPlayers()) {
			if (shown == MAX_NEARBY || y + 18 > py + ph - 8) {
				return;
			}
			final String id = player[0].toString();
			boolean hidden = config.hiddenPlayers.contains(id);
			if (!hidden && ArcticClient.looks().lookFor((java.util.UUID) player[0]) == null) {
				continue;
			}
			String label = (hidden ? "Show " : "Hide ") + player[1] + "'s look";
			add(new Button(label, new Runnable() {
				@Override
				public void run() {
					if (!config.hiddenPlayers.remove(id)) {
						config.hiddenPlayers.add(id);
					}
					ArcticClient.saveConfig();
					rebuild();
				}
			})).bounds(contentX(), y, contentW(), 18);
			y += 20;
			shown++;
		}
	}

	// ---- Style -----------------------------------------------------------------

	private void buildStyle() {
		Style[] all = Style.ALL;
		int gap = 6;
		int cardW = (contentW() - gap * (all.length - 1)) / all.length;
		for (int i = 0; i < all.length; i++) {
			final Style style = all[i];
			boolean chosen = style.id.equals(ArcticClient.style().id);
			add(new StyleCard(style, chosen, new Runnable() {
				@Override
				public void run() {
					ArcticClient.config().style = style.id;
					ArcticClient.saveConfig();
					rebuild();
				}
			})).bounds(contentX() + i * (cardW + gap), contentTop(), cardW, 66);
		}
	}

	/** Everything the Looks tab shows; lookups in the background don't count. */
	private static String looksState() {
		Looks looks = ArcticClient.looks();
		Look mine = looks.myLook();
		StringBuilder state = new StringBuilder();
		state.append(looks.busy()).append('|').append(looks.signedIn()).append('|').append(looks.status());
		state.append('|').append(mine == null ? null : mine.cape);
		for (Preset p : looks.presets()) {
			state.append('|').append(p.id);
		}
		return state.toString();
	}

	@Override
	public void tick() {
		if (tab == Tab.LOOKS && !shownLooks.equals(looksState())) {
			rebuild();
		}
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		Skin.panel(g, s, px, py, px + pw, py + ph);
		g.fill(px + SIDEBAR, py + 8, px + SIDEBAR + 1, py + ph - 8, s.border);
		g.texture("icon", px + 10, py + 10, 14, 14, 0, 0, 256, 256, 256, 256);
		g.text("§lArctic", px + 28, py + 13, s.text, false);
		g.text(tab.title, contentX(), py + 10, s.text, false);
		int room = tab == Tab.HUD ? contentW() - 90 : contentW();
		g.text(Draw.fit(g, status(), room), contentX(), py + 22, s.muted, false);
		if (tab == Tab.STYLE) {
			Style chosen = ArcticClient.style();
			int y = contentTop() + 74;
			g.text(chosen.name, contentX(), y, s.accent, false);
			g.text(Draw.fit(g, chosen.description, contentW()), contentX(), y + 12, s.muted, false);
			g.text("Changes apply right away, to every menu.", contentX(), y + 30, s.muted, false);
		}
	}

	private String status() {
		if (tab == Tab.LOOKS && !ArcticClient.looks().status().isEmpty()) {
			return ArcticClient.looks().status();
		}
		return tab.hint;
	}

	@Override
	public void close() {
		ArcticClient.saveConfig();
		super.close();
	}
}
