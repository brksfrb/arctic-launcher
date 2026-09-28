package com.arcticlauncher.client.menu;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.packs.PackBrowser;
import com.arcticlauncher.client.packs.PackIcons;
import com.arcticlauncher.client.packs.PackInfo;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Scroll;
import com.arcticlauncher.client.ui.TextField;
import com.arcticlauncher.client.ui.Toggle;

/**
 * Resource packs: yours (turn them on and off, change their order, then
 * Apply) and Modrinth's (search as you type, install and turn on in one
 * click; no restart).
 */
final class PacksTab implements MenuTab {
	private static final String[] VIEWS = {"Your packs", "Browse Modrinth"};
	private static final int CHIP_H = 16;
	private static final int CHIP_W = 110;
	private static final int FIELD_H = 18;
	private static final int BTN_H = 16;
	private static final int SWITCH_W = 40;
	private static final int ARROW_W = 16;
	private static final int ACTION_W = 66;
	private static final int GAP = 4;
	private static final int MAX_QUERY = 64;
	/** Wait this long after the last key before searching. */
	private static final long SEARCH_DELAY_MS = 350;

	/** Kept while the game runs. */
	private static int view;
	private static String query = "";
	/** What you've set (on packs, top first); null = nothing changed yet. */
	private static List<String> pending;

	private final TextField search = new TextField("Search Modrinth packs", MAX_QUERY, TextField.ANY);
	private long typedAt;
	/** Your packs as the game has them now. */
	private List<PackInfo> packs = new ArrayList<PackInfo>();
	private volatile String applying;
	private int x;
	private int w;
	private int listTop;
	private boolean empty;

	/** Open on a view (0 yours, 1 Modrinth) with a search typed in. */
	static void show(int v, String search) {
		view = v;
		query = search == null ? "" : search;
	}

	PacksTab() {
		search.text(query);
		search.onChange(() -> {
			query = search.text();
			typedAt = System.currentTimeMillis();
		});
	}

	@Override
	public String title() {
		return "Resource Packs";
	}

	@Override
	public String hint() {
		return "Turn packs on and off, or find new ones on Modrinth.";
	}

	@Override
	public String status() {
		if (applying != null) {
			return applying;
		}
		String s = ArcticClient.packs().status();
		return view == 1 && !s.isEmpty() ? s : null;
	}

	@Override
	public boolean rebuildWhileTyping() {
		return true;
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		this.x = x;
		this.w = w;
		for (int i = 0; i < VIEWS.length; i++) {
			final int v = i;
			host.add(new Button(VIEWS[i], () -> {
				view = v;
				host.rebuild();
			}).selected(view == i)).bounds(x + i * (CHIP_W + GAP), top, CHIP_W, CHIP_H);
		}
		final java.io.File folder = ArcticClient.platform().resourcePackDir();
		host.add(new Button("Open folder", () -> openFolder(folder))).bounds(x + w - 80, top, 80, CHIP_H);
		int y = top + CHIP_H + 8;
		listTop = y;
		packs = ArcticClient.platform().resourcePacks();
		if (view == 0) {
			yours(host, y, bottom);
		} else {
			browse(host, y, bottom);
		}
	}

	// ---- Your packs ----------------------------------------------------------------

	private List<String> enabledNow() {
		List<String> on = new ArrayList<String>();
		for (PackInfo p : packs) {
			if (p.enabled) {
				on.add(p.id);
			}
		}
		return on;
	}

	private void yours(final Host host, int y, int bottom) {
		final List<String> current = enabledNow();
		final List<String> wanted = pending != null ? pending : current;
		boolean changed = pending != null && !pending.equals(current);
		int listBottom = bottom - (changed ? BTN_H + 8 : 0);
		Scroll scroll = host.scroll(x, y, x + w, listBottom);
		// On ones in your order first, then the rest.
		List<PackInfo> ordered = new ArrayList<PackInfo>();
		for (String id : wanted) {
			for (PackInfo p : packs) {
				if (p.id.equals(id)) {
					ordered.add(p);
				}
			}
		}
		for (PackInfo p : packs) {
			if (!wanted.contains(p.id)) {
				ordered.add(p);
			}
		}
		empty = ordered.isEmpty();
		final PackIcons icons = ArcticClient.packIcons();
		int rowW = w - 8;
		int ry = y;
		for (final PackInfo p : ordered) {
			final boolean on = wanted.contains(p.id);
			final int at = wanted.indexOf(p.id);
			int right = SWITCH_W + (on ? (ARROW_W + 2) * 2 : 0) + 4;
			scroll.add(host.add(new PackRow(p.title, p.description.replace('\n', ' '), () -> icons.local(p.file), right, !on)
					.bounds(x, ry, rowW, PackRow.H)));
			Toggle t = new Toggle("", null, Form.binding(() -> on, v -> {
				List<String> next = new ArrayList<String>(wanted);
				if (v) {
					next.add(0, p.id);
				} else {
					next.remove(p.id);
				}
				pending = next;
				host.rebuild();
			}));
			t.enabled = !p.required;
			scroll.add(host.add(t.bounds(x + rowW - SWITCH_W, ry + (PackRow.H - 16) / 2, SWITCH_W, 16)));
			if (on) {
				int ax = x + rowW - SWITCH_W - (ARROW_W + 2) * 2;
				Button up = new Button("▲", () -> move(wanted, at, -1, host));
				up.enabled = at > 0;
				Button down = new Button("▼", () -> move(wanted, at, 1, host));
				down.enabled = at < wanted.size() - 1;
				scroll.add(host.add(up.bounds(ax, ry + (PackRow.H - BTN_H) / 2, ARROW_W, BTN_H)));
				scroll.add(host.add(down.bounds(ax + ARROW_W + 2, ry + (PackRow.H - BTN_H) / 2, ARROW_W, BTN_H)));
			}
			ry += PackRow.H + 2;
		}
		scroll.settle();
		if (changed) {
			host.add(new Button("Apply changes", () -> apply(host)).primary()).bounds(x + w - 110, bottom - BTN_H, 110, BTN_H);
			host.add(new Button("Undo", () -> {
				pending = null;
				host.rebuild();
			})).bounds(x + w - 110 - GAP - 60, bottom - BTN_H, 60, BTN_H);
		}
	}

	private static void move(List<String> wanted, int at, int by, Host host) {
		List<String> next = new ArrayList<String>(wanted);
		int to = at + by;
		if (at < 0 || to < 0 || to >= next.size()) {
			return;
		}
		next.add(to, next.remove(at));
		pending = next;
		host.rebuild();
	}

	private void apply(final Host host) {
		final List<String> wanted = pending;
		pending = null;
		applying = "Loading the packs… (a few seconds)";
		final Platform p = ArcticClient.platform();
		p.runOnGameThread(() -> {
			p.setResourcePacks(wanted);
			applying = null;
			host.rebuild();
		});
	}

	// ---- Browse ----------------------------------------------------------------------

	private void browse(final Host host, int y, int bottom) {
		final PackBrowser browser = ArcticClient.packs();
		browser.search(query);
		host.add(search).bounds(x, y, w, FIELD_H);
		listTop = y + FIELD_H + 6;
		Scroll scroll = host.scroll(x, listTop, x + w, bottom);
		final PackIcons icons = ArcticClient.packIcons();
		List<PackBrowser.Pack> list = browser.results();
		empty = list.isEmpty();
		int rowW = w - 8;
		int ry = listTop;
		for (final PackBrowser.Pack pack : list) {
			String by = "by " + pack.author + " · " + downloads(pack.downloads) + " · " + pack.description;
			scroll.add(host.add(new PackRow(pack.title, by, () -> icons.remote(pack.iconUrl), ACTION_W + 4, false)
					.bounds(x, ry, rowW, PackRow.H)));
			scroll.add(host.add(action(host, browser, pack).bounds(x + rowW - ACTION_W, ry + (PackRow.H - BTN_H) / 2, ACTION_W, BTN_H)));
			ry += PackRow.H + 2;
		}
		if (browser.hasMore()) {
			scroll.add(host.add(new Button(browser.searching() ? "Loading…" : "Show more", browser::loadMore)
					.enabled(!browser.searching()).bounds(x + (w - 120) / 2, ry + 4, 120, BTN_H)));
		}
		scroll.settle();
	}

	/** Install, Installing…, Turn on, or On (click to turn off). */
	private Button action(final Host host, final PackBrowser browser, final PackBrowser.Pack pack) {
		if (pack.id.equals(browser.busy())) {
			return new Button("Installing…", () -> {}).enabled(false);
		}
		final String file = browser.installedFile(pack.id);
		if (file == null) {
			return new Button("Install", () -> browser.install(pack)).primary().enabled(browser.busy() == null);
		}
		final PackInfo info = byFile(file);
		if (info != null && info.enabled) {
			return new Button("On", () -> {
				List<String> next = enabledNow();
				next.remove(info.id);
				pending = next;
				apply(host);
			}).selected(true);
		}
		return new Button("Turn on", () -> {
			if (info == null) {
				ArcticClient.platform().runOnGameThread(() -> ArcticClient.platform().enableResourcePack(file));
				return;
			}
			List<String> next = enabledNow();
			next.add(0, info.id);
			pending = next;
			apply(host);
		});
	}

	private PackInfo byFile(String name) {
		for (PackInfo p : packs) {
			if (p.file != null && p.file.getName().equals(name)) {
				return p;
			}
		}
		return null;
	}

	private static String downloads(int n) {
		if (n >= 1_000_000) {
			return String.format(Locale.ROOT, "%.1fM downloads", n / 1e6);
		}
		return n >= 1000 ? n / 1000 + "k downloads" : n + " downloads";
	}

	private static void openFolder(java.io.File folder) {
		try {
			folder.mkdirs();
			java.awt.Desktop.getDesktop().open(folder);
		} catch (Exception e) {
			com.arcticlauncher.client.notice.Notices.post("Couldn't open the folder", folder.getPath());
		}
	}

	@Override
	public void tick(Host host) {
		if (view == 1 && typedAt > 0 && System.currentTimeMillis() - typedAt > SEARCH_DELAY_MS) {
			typedAt = 0;
			ArcticClient.packs().search(query);
		}
	}

	@Override
	public void draw(Gfx g, Style s, int mx, int my) {
		if (!empty) {
			return;
		}
		String text = view == 0 ? "No packs yet. Browse Modrinth, or drop zips in the folder."
				: ArcticClient.packs().searching() ? "Searching…" : "";
		Draw.centered(g, text, x + w / 2, listTop + 30, s.muted, false);
	}

	@Override
	public String state() {
		PackBrowser b = ArcticClient.packs();
		StringBuilder out = new StringBuilder().append(view).append('/').append(b.busy()).append('/').append(b.searching())
				.append('/').append(b.hasMore()).append('/').append(applying).append('/');
		for (PackBrowser.Pack p : b.results()) {
			out.append(p.id).append(',');
		}
		return out.toString();
	}
}
