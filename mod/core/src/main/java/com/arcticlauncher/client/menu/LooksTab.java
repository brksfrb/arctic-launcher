package com.arcticlauncher.client.menu;

import java.util.List;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.looks.Look;
import com.arcticlauncher.client.looks.Looks;
import com.arcticlauncher.client.looks.Preset;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.KeyButton;
import com.arcticlauncher.client.ui.Toggle;

/** Looks: capes, cosmetics and emotes, with you (live) on the right. */
final class LooksTab implements MenuTab {
	private static final String[] PAGES = {"Capes", "Cosmetics", "Emotes"};
	private static final int PAGE_BAR = 20;
	private static final int PREVIEW = 90;
	private static final int ROW = 24;
	private static final int KEY_W = 76;
	private static final int TILE_W = 50;
	private static final int TILE_H = 58;
	private static final int MAX_NEARBY = 3;

	/** Remembered while the game runs. */
	static int page;
	private boolean requestedCapes;
	private int x;
	private int top;
	private int w;
	private int bottom;
	private int wheelKeyY = -1;

	@Override
	public String title() {
		return "Looks";
	}

	@Override
	public String hint() {
		return "Every Arctic player sees your cape and cosmetics.";
	}

	@Override
	public String status() {
		String s = ArcticClient.looks().status();
		return s.isEmpty() ? null : s;
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		this.x = x;
		this.top = top;
		this.w = w;
		this.bottom = bottom;
		wheelKeyY = -1;
		Looks looks = ArcticClient.looks();
		if (!requestedCapes && looks.presets().isEmpty() && !looks.busy()) {
			requestedCapes = true;
			looks.open();
		}
		int pageW = (w - PREVIEW - 8 - 2 * 4) / PAGES.length;
		for (int i = 0; i < PAGES.length; i++) {
			final int p = i;
			host.add(new Button(PAGES[i], () -> {
				page = p;
				host.rebuild();
			}).selected(i == page)).bounds(x + i * (pageW + 4), top, pageW, 16);
		}
		int listTop = top + PAGE_BAR;
		int listW = w - PREVIEW - 8;
		CosmeticTiles.Host tiles = host::add;
		if (page == 1) {
			CosmeticTiles.cosmetics(tiles, x, listTop, listW);
			return;
		}
		if (page == 2) {
			int end = CosmeticTiles.emotes(tiles, x, listTop, listW);
			final ClientConfig c = ArcticClient.config();
			KeyButton wheelKey = new KeyButton(Form.key(() -> c.emoteKey, k -> {
				c.emoteKey = k;
				ArcticClient.saveConfig();
			}));
			host.listenKeys(wheelKey);
			wheelKeyY = end + 6;
			host.add(wheelKey).bounds(x + 80, wheelKeyY, KEY_W, 18);
			return;
		}
		int end = capes(host, looks, listTop, listW);
		if (looks.presets().isEmpty() && !looks.busy()) {
			host.add(new Button("Try again", () -> ArcticClient.looks().open())).bounds(x, end + 4, 80, 18);
			end += 24;
		}
		final ClientConfig config = ArcticClient.config();
		host.add(new Toggle("Show Arctic looks", "Other players' Arctic skins, capes and cosmetics",
				Form.binding(() -> config.showCosmetics, on -> config.showCosmetics = on))).bounds(x, end + 8, listW, ROW);
		nearby(host, end + 8 + ROW + 4, listW);
	}

	/** The cape grid; returns its bottom edge. */
	private int capes(Host host, Looks looks, int y0, int listW) {
		List<Preset> presets = looks.presets();
		Look mine = looks.myLook();
		String worn = mine == null ? null : mine.cape;
		int count = presets.size() + 1;
		int cols = Math.max(1, Math.min(count, listW / TILE_W));
		int tileW = (listW - (cols - 1) * 4) / cols;
		boolean canWear = looks.signedIn() && !looks.busy();
		for (int i = 0; i < count; i++) {
			Preset p = i == 0 ? null : presets.get(i - 1);
			String texture = p == null ? null : p.texture;
			boolean isWorn = texture == null ? worn == null : texture.equals(worn);
			CapeTile tile = new CapeTile(p == null ? null : p.id, p == null ? "No cape" : p.name, texture, isWorn);
			tile.enabled = canWear;
			host.add(tile).bounds(x + (i % cols) * (tileW + 4), y0 + (i / cols) * (TILE_H + 4), tileW, TILE_H);
		}
		int rows = (count + cols - 1) / cols;
		return y0 + rows * (TILE_H + 4) - 4;
	}

	/** Hide or show a nearby player's look. */
	private void nearby(final Host host, int y, int listW) {
		final ClientConfig config = ArcticClient.config();
		int shown = 0;
		for (Object[] player : ArcticClient.platform().otherPlayers()) {
			if (shown == MAX_NEARBY || y + 18 > bottom) {
				return;
			}
			final String id = player[0].toString();
			boolean hidden = config.hiddenPlayers.contains(id);
			if (!hidden && ArcticClient.looks().lookFor((java.util.UUID) player[0]) == null) {
				continue;
			}
			host.add(new Button((hidden ? "Show " : "Hide ") + player[1] + "'s look", () -> {
				if (!config.hiddenPlayers.remove(id)) {
					config.hiddenPlayers.add(id);
				}
				ArcticClient.saveConfig();
				host.rebuild();
			})).bounds(x, y, listW, 18);
			y += 20;
			shown++;
		}
	}

	/** You, live, wearing your cosmetics (and playing emotes). */
	@Override
	public void draw(Gfx g, Style s, int mx, int my) {
		int x1 = x + w;
		int x0 = x1 - PREVIEW;
		int y0 = top;
		int y1 = bottom;
		Skin.panel(g, s, x0, y0, x1, y1);
		if (!ArcticClient.platform().inWorld()) {
			Draw.centered(g, "In a world", (x0 + x1) / 2, (y0 + y1) / 2 - 4, s.muted, false);
		} else {
			g.player(x0 + 2, y0 + 2, x1 - 2, y1 - 2, (y1 - y0) / 3, mx, my);
		}
		if (wheelKeyY >= 0) {
			g.text("Wheel key", x, wheelKeyY + 5, s.muted, false);
		}
	}

	@Override
	public String state() {
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
}
