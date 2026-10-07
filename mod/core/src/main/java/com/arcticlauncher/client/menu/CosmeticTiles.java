package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.looks.Cosmetics;
import com.arcticlauncher.client.looks.Look;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Widget;
import java.util.ArrayList;
import java.util.List;

/** Tiles for the Looks tab's Cosmetics and Emotes pages. */
final class CosmeticTiles {
	private static final int TILE_H = 30;
	private static final int GAP = 4;

	private CosmeticTiles() {}

	/** Where the pages put their widgets. */
	interface Host {
		Widget add(Widget w);
	}

	/** One cosmetic: click to wear it (replacing the one in its slot) or take it off. */
	static final class CosmeticTile extends Widget {
		private final Cosmetics.Item item;
		private final boolean worn;

		CosmeticTile(Cosmetics.Item item, boolean worn) {
			this.item = item;
			this.worn = worn;
		}

		@Override
		protected void draw(Gfx g, Style s, int mx, int my, float dt) {
			if (contains(mx, my)) {
				// Try it on: you on the right wear it while it's pointed at.
				ArcticClient.looks().tryOn(after());
			}
			Skin.button(g, s, x, y, w, h, enabled, worn ? 1f : hover);
			if (worn) {
				Draw.outline(g, x, y, x + w, y + h, 2, s.accent);
			}
			g.text(Draw.fit(g, item.name, w - 8, x + 4, y + 5), x + 4, y + 5, worn ? s.accent : s.text, false);
			g.text(Draw.fit(g, worn ? slotName(item.slot) + " · worn" : slotName(item.slot), w - 8), x + 4, y + 17, s.muted, false);
		}

		@Override
		public boolean click(double mx, double my, int button) {
			if (!enabled || button != Keys.MOUSE_LEFT) {
				return false;
			}
			ArcticClient.looks().wearCosmetics(after());
			return true;
		}

		/** What you'd wear after clicking this: the rest, plus this in its slot (or nothing there). */
		private List<String> after() {
			Look mine = ArcticClient.looks().myLook();
			List<String> next = new ArrayList<String>();
			Cosmetics cosmetics = ArcticClient.looks().cosmetics();
			for (String id : mine == null ? new ArrayList<String>() : mine.cosmetics) {
				Cosmetics.Item other = cosmetics.item(id);
				if (other != null && !other.slot.equals(item.slot)) {
					next.add(id);
				}
			}
			if (!worn) {
				next.add(item.id);
			}
			return next;
		}
	}

	/** One emote: click to play it. */
	static final class EmoteTile extends Widget {
		/** The emote being previewed on you (by hovering), across tiles. */
		private static Cosmetics.Emote previewing;
		private final Cosmetics.Emote emote;

		EmoteTile(Cosmetics.Emote emote) {
			this.emote = emote;
		}

		@Override
		protected void draw(Gfx g, Style s, int mx, int my, float dt) {
			java.util.UUID me = ArcticClient.platform().worldPlayerId();
			boolean over = contains(mx, my);
			if (me != null && over && previewing != emote) {
				// Pointed at: you on the right do it (only you see a preview).
				previewing = emote;
				ArcticClient.looks().cosmetics().playLocal(me, emote);
			} else if (me != null && !over && previewing == emote) {
				previewing = null;
				ArcticClient.looks().cosmetics().playLocal(me, null);
			}
			Skin.button(g, s, x, y, w, h, enabled, hover);
			Draw.centered(g, Draw.fitCentered(g, emote.name, w - 6, x + w / 2, y + (h - 8) / 2), x + w / 2, y + (h - 8) / 2, s.text, false);
		}

		@Override
		public boolean click(double mx, double my, int button) {
			if (!enabled || button != Keys.MOUSE_LEFT) {
				return false;
			}
			previewing = null;
			ArcticClient.looks().playEmote(emote);
			return true;
		}
	}

	/** The cosmetics grid; returns its bottom edge. */
	static int cosmetics(Host host, int x, int top, int w) {
		Cosmetics cosmetics = ArcticClient.looks().cosmetics();
		Look mine = ArcticClient.looks().myLook();
		List<Cosmetics.Item> items = cosmetics.items();
		int cols = Math.max(1, w / 110);
		int tileW = (w - (cols - 1) * GAP) / cols;
		// Clicks while a save is going are queued (the newest is sent next).
		boolean canWear = ArcticClient.looks().signedIn();
		for (int i = 0; i < items.size(); i++) {
			Cosmetics.Item item = items.get(i);
			boolean worn = mine != null && mine.cosmetics.contains(item.id);
			CosmeticTile tile = new CosmeticTile(item, worn);
			tile.enabled = canWear;
			host.add(tile).bounds(x + (i % cols) * (tileW + GAP), top + (i / cols) * (TILE_H + GAP), tileW, TILE_H);
		}
		int rows = (items.size() + cols - 1) / cols;
		int end = top + rows * (TILE_H + GAP);
		if (mine != null && !mine.cosmetics.isEmpty()) {
			com.arcticlauncher.client.ui.Button off = new com.arcticlauncher.client.ui.Button("Remove all",
					() -> ArcticClient.looks().wearCosmetics(java.util.Collections.<String>emptyList()));
			off.enabled = canWear;
			host.add(off).bounds(x, end, 90, 18);
			end += 18 + GAP;
		}
		return end;
	}

	/** The emotes grid; returns its bottom edge. */
	static int emotes(Host host, int x, int top, int w) {
		List<Cosmetics.Emote> emotes = ArcticClient.looks().cosmetics().emotes();
		int cols = Math.max(1, w / 80);
		int tileW = (w - (cols - 1) * GAP) / cols;
		boolean inWorld = ArcticClient.platform().inWorld();
		for (int i = 0; i < emotes.size(); i++) {
			EmoteTile tile = new EmoteTile(emotes.get(i));
			tile.enabled = inWorld;
			host.add(tile).bounds(x + (i % cols) * (tileW + GAP), top + (i / cols) * (20 + GAP), tileW, 20);
		}
		int rows = (emotes.size() + cols - 1) / cols;
		return top + rows * (20 + GAP);
	}

	static String slotName(String slot) {
		switch (slot) {
			case "head":
				return "Head";
			case "face":
				return "Face";
			case "back":
				return "Back";
			case "shoulders":
				return "Shoulders";
			case "arms":
				return "Arms";
			default:
				return "Body";
		}
	}
}
