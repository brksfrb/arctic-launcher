package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.ui.Page;

/**
 * Where the Arctic menu opens: Right Shift in a world shows the HUD screen
 * (drag widgets, then Mods); elsewhere it's straight to the Mods screen.
 * Pages can also be opened by name ("friends", "packs", a mod's id…).
 */
public final class Menus {
	private Menus() {}

	/** What Right Shift opens right now. */
	public static Page home() {
		if (ArcticClient.platform().inWorld() && ArcticClient.platform().hasFeatures()) {
			return new HudEditor();
		}
		return new ModsScreen();
	}

	public static void open() {
		ArcticClient.platform().openPage(home());
	}

	public static void mods() {
		ArcticClient.platform().openPage(new ModsScreen());
	}

	/** A page by name: a mod's id ("zoom", "hud.fps", "packs", "friends", …), or "mods". */
	public static Page page(String id) {
		return page(id, null);
	}

	/** {@link #page(String)}, whose back arrow returns to {@code from}. */
	static Page page(String id, Page from) {
		if ("mods".equals(id)) {
			return new ModsScreen();
		}
		for (Mod m : ModCatalog.all()) {
			if (m.id.equals(id)) {
				return pageFor(m, from);
			}
		}
		return new ModsScreen();
	}

	public static void open(String id) {
		ArcticClient.platform().openPage(page(id));
	}

	/** Open a page whose back arrow returns to {@code from}. */
	static void open(String id, Page from) {
		ArcticClient.platform().openPage(page(id, from));
	}

	/** For tests: the page {@link #selected()} opens, by an old tab's name too ("world", "features"…). */
	private static String selectedId = "mods";
	private static final String[][] OLD_TABS = {
			{"hud", "mods"}, {"features", "zoom"}, {"view", "chat"}, {"world", "outline"}, {"messages", "messages"},
	};

	public static void select(String id) {
		String mapped = id;
		for (String[] pair : OLD_TABS) {
			if (pair[0].equals(id)) {
				mapped = pair[1];
			}
		}
		selectedId = mapped;
	}

	/** Looks on a page (0 capes, 1 cosmetics, 2 emotes). */
	public static void selectLooks(int page) {
		LooksTab.page = Math.max(0, Math.min(2, page));
		selectedId = "looks";
	}

	/** Packs on your packs or the Modrinth browser (with a search typed in). */
	public static void selectPacks(boolean browse, String search) {
		PacksTab.show(browse ? 1 : 0, search);
		selectedId = "packs";
	}

	public static Page selected() {
		return page(selectedId);
	}

	static Page pageFor(Mod m, Page from) {
		MenuTab tab = m.page != null ? m.page.get() : new ModSettingsTab(m);
		return new TabPage(tab, m, from);
	}
}
