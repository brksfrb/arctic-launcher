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
import com.arcticlauncher.client.packs.PackGroups;
import com.arcticlauncher.client.packs.PackInfo;
import com.arcticlauncher.client.packs.ShaderPacks;
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
	private static boolean shader;
	/** The group the list is narrowed to: "" = every pack, NO_GROUP = the ones in none. */
	private static String groupFilter = "";
	private static final String NO_GROUP = "\u0000none";
	private static final int GROUP_W = 44;
	private static final String[] QUERIES = {"", ""};
	private static final int[] SORT = {0, 0}, CATEGORY = {0, 0}, DETAIL = {0, 0};
	private static final boolean[] THIS_VERSION = {true, true};
	private static final String[] SORT_IDS = {"downloads", "relevance", "updated", "newest"};
	private static final String[] SORT_LABELS = {"Most downloaded", "Best match", "Recently updated", "Newest"};
	private static final String[][] CATEGORIES = {
			{"", "combat", "cursed", "decoration", "modded", "realistic", "simplistic", "themed", "tweaks", "utility", "vanilla-like"},
			{"", "cartoon", "cursed", "fantasy", "realistic", "semi-realistic", "vanilla-like"}};
	private static final String[][] DETAILS = {
			{"", "8x-", "16x", "32x", "48x", "64x", "128x", "256x", "512x+"},
			{"", "potato", "low", "medium", "high", "screenshot"}};
	private int expandedFilter;
	private Scroll currentScroll;
	private long refreshedAt;
	/** What you've set (on packs, top first); null = nothing changed yet. */
	private static List<String> pending;

	private final TextField search = new TextField("Search Modrinth packs", MAX_QUERY, TextField.ANY);
	/** A new group's name, typed before its first pack's Group button is clicked. */
	private final TextField newGroup = new TextField("New group name", 24, TextField.ANY);
	private PackGroups groups;
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
		shader = false;
		view = v;
		query = search == null ? "" : search;
		QUERIES[0] = query;
	}

	static void showShaders(int v, String search) {
		shader = true;
		view = v;
		query = search == null ? "" : search;
		QUERIES[1] = query;
	}

	private static int kind() {
		return shader ? 1 : 0;
	}

	private static PackBrowser browser() {
		return shader ? ArcticClient.shaderBrowser() : ArcticClient.packs();
	}

	private static PackBrowser.Filters filters() {
		int k = kind();
		return new PackBrowser.Filters(SORT_IDS[SORT[k]], THIS_VERSION[k], CATEGORIES[k][CATEGORY[k]], DETAILS[k][DETAIL[k]]);
	}

	PacksTab() {
		search.text(query);
		search.onChange(() -> {
			query = search.text();
			QUERIES[kind()] = query;
			typedAt = System.currentTimeMillis();
		});
	}

	@Override
	public String title() {
		return "Packs & Shaders";
	}

	@Override
	public String hint() {
		return shader && !ArcticClient.shaders().available() ? ShaderPacks.NEEDS_IRIS
				: "Turn packs on and off, or find new ones on Modrinth.";
	}

	@Override
	public String status() {
		if (applying != null) {
			return applying;
		}
		if (shader && ArcticClient.shaders().changing()) {
			return ArcticClient.shaders().status();
		}
		String s = browser().status();
		if (shader && browser().busy() == null && !ArcticClient.shaders().status().isEmpty()) {
			return ArcticClient.shaders().status();
		}
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
		for (int i = 0; i < 2; i++) {
			final boolean shaders = i == 1;
			host.add(new Button(shaders ? "Shaders" : "Resource packs", () -> {
				shader = shaders;
				query = QUERIES[kind()];
				search.text(query);
				typedAt = 0;
				expandedFilter = 0;
				resetScroll();
				host.rebuild();
			}).selected(shader == shaders)).bounds(x + i * (CHIP_W + GAP), top, CHIP_W, CHIP_H);
		}
		top += CHIP_H + GAP;
		int viewWidth = Math.min(CHIP_W, (w - 80 - GAP * 2) / 2);
		for (int i = 0; i < VIEWS.length; i++) {
			final int v = i;
			host.add(new Button(VIEWS[i], () -> {
				view = v;
				resetScroll();
				host.rebuild();
			}).selected(view == i)).bounds(x + i * (viewWidth + GAP), top, viewWidth, CHIP_H);
		}
		final java.io.File folder = browser().folder();
		host.add(new Button("Open folder", () -> openFolder(folder))).bounds(x + w - 80, top, 80, CHIP_H);
		int y = top + CHIP_H + 8;
		listTop = y;
		packs = shader ? new ArrayList<PackInfo>() : ArcticClient.platform().resourcePacks();
		if (view == 0) {
			if (shader) {
				shaders(host, y, bottom);
			} else {
				yours(host, y, bottom);
			}
		} else {
			browse(host, y, bottom);
		}
	}

	// ---- Groups --------------------------------------------------------------------

	private String groupKind() {
		return shader ? PackGroups.SHADER : PackGroups.RESOURCE;
	}

	/** Does a pack in {@code group} show under the chosen filter? */
	private static boolean inFilter(String group) {
		if (groupFilter.isEmpty()) {
			return true;
		}
		return NO_GROUP.equals(groupFilter) ? group == null : groupFilter.equalsIgnoreCase(group);
	}

	/** "All, No group, each group" chips and the new-group box; returns where the list starts. */
	private int groupBar(final Host host, int y) {
		groups = PackGroups.load(ArcticClient.platform().configDir());
		List<String> names = groups.names(groupKind());
		if (!groupFilter.isEmpty() && !NO_GROUP.equals(groupFilter) && !names.contains(groupFilter)) {
			groupFilter = "";
		}
		List<String> ids = new ArrayList<String>();
		List<String> labels = new ArrayList<String>();
		ids.add("");
		labels.add("All");
		if (!names.isEmpty()) {
			ids.add(NO_GROUP);
			labels.add("No group");
		}
		for (String name : names) {
			ids.add(name);
			labels.add(name);
		}
		int field = 96;
		int room = w - field - GAP;
		int cw = Math.max(40, Math.min(90, (room - GAP * (ids.size() - 1)) / ids.size()));
		for (int i = 0; i < ids.size() && (i + 1) * (cw + GAP) <= room + GAP; i++) {
			final String id = ids.get(i);
			host.add(new Button(labels.get(i), () -> {
				groupFilter = id;
				resetScroll();
				host.rebuild();
			}).selected(groupFilter.equals(id))).bounds(x + i * (cw + GAP), y, cw, CHIP_H);
		}
		host.add(newGroup).bounds(x + w - field, y, field, CHIP_H);
		return y + CHIP_H + GAP;
	}

	/** The row's Group button: moves the pack to the next group, or to the one typed in the box. */
	private Button groupButton(final Host host, final String fileName, final String current) {
		final String typed = newGroup.text().trim();
		String label = typed.isEmpty() ? (current == null ? "Group" : "Next") : "Set";
		return new Button(label, () -> {
			String target = typed.isEmpty() ? groups.next(groupKind(), current) : typed;
			groups.set(groupKind(), fileName, target);
			newGroup.text("");
			host.rebuild();
		});
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
		y = groupBar(host, y);
		int listBottom = bottom - (changed ? BTN_H + 8 : 0);
		Scroll scroll = currentScroll = host.scroll(x, y, x + w, listBottom);
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
		// Narrowed to a group: packs without a file (the game's own) belong to none.
		for (int i = ordered.size() - 1; i >= 0; i--) {
			PackInfo p = ordered.get(i);
			String group = p.file == null ? null : groups.groupOf(PackGroups.RESOURCE, p.file.getName());
			if (!inFilter(group)) {
				ordered.remove(i);
			}
		}
		empty = ordered.isEmpty();
		final PackIcons icons = ArcticClient.packIcons();
		int rowW = w - 8;
		int ry = y;
		for (final PackInfo p : ordered) {
			final boolean on = wanted.contains(p.id);
			final int at = wanted.indexOf(p.id);
			final String group = p.file == null ? null : groups.groupOf(PackGroups.RESOURCE, p.file.getName());
			int arrows = on ? (ARROW_W + 2) * 2 : 0;
			int right = SWITCH_W + arrows + (p.file == null ? 0 : GROUP_W + 2) + 4;
			String line = (group == null ? "" : "[" + group + "] ") + p.description.replace('\n', ' ');
			scroll.add(host.add(new PackRow(p.title, line, () -> icons.local(p.file), right, !on)
					.bounds(x, ry, rowW, PackRow.H)));
			if (p.file != null) {
				int gx = x + rowW - SWITCH_W - arrows - GROUP_W - 2;
				scroll.add(host.add(groupButton(host, p.file.getName(), group)
						.bounds(gx, ry + (PackRow.H - BTN_H) / 2, GROUP_W, BTN_H)));
			}
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

	private void resetScroll() {
		if (currentScroll != null) {
			currentScroll.scrollTo(0);
		}
	}

	private void shaders(final Host host, int y, int bottom) {
		ShaderPacks shaders = ArcticClient.shaders();
		y = groupBar(host, y);
		Scroll scroll = currentScroll = host.scroll(x, y, x + w, bottom);
		String active = shaders.active();
		List<java.io.File> files = new ArrayList<java.io.File>();
		for (java.io.File candidate : shaders.installed()) {
			if (inFilter(groups.groupOf(PackGroups.SHADER, candidate.getName()))) {
				files.add(candidate);
			}
		}
		empty = files.isEmpty();
		for (java.io.File file : files) {
			String name = file.getName();
			boolean on = name.equals(active);
			String group = groups.groupOf(PackGroups.SHADER, name);
			String line = (group == null ? "" : "[" + group + "] ") + (on ? "Active shader" : "Installed shader");
			scroll.add(host.add(new PackRow(name, line, () -> null, ACTION_W + GROUP_W + 8, !on)
					.bounds(x, y, w - 8, PackRow.H)));
			scroll.add(host.add(groupButton(host, name, group)
					.bounds(x + w - 8 - ACTION_W - GROUP_W - 4, y + (PackRow.H - BTN_H) / 2, GROUP_W, BTN_H)));
			scroll.add(host.add(new Button(on ? "Turn off" : "Turn on", () -> shaders.select(on ? null : name))
					.selected(on).enabled(shaders.available() && !shaders.changing())
					.bounds(x + w - 8 - ACTION_W, y + (PackRow.H - BTN_H) / 2, ACTION_W, BTN_H)));
			y += PackRow.H + 2;
		}
		scroll.settle();
	}

	private void browse(final Host host, int y, int bottom) {
		final PackBrowser browser = browser();
		if (typedAt == 0) {
			browser.search(query, filters());
		}
		host.add(search).bounds(x, y, w - 60, FIELD_H);
		host.add(new Button("Refresh", browser::retry).enabled(!browser.searching())).bounds(x + w - 56, y, 56, FIELD_H);
		y += FIELD_H + GAP;
		final int k = kind();
		int cw = (w - GAP * 2) / 3;
		String[] titles = {SORT_LABELS[SORT[k]], "Category: " + label(CATEGORIES[k][CATEGORY[k]]),
				(shader ? "Impact: " : "Resolution: ") + label(DETAILS[k][DETAIL[k]])};
		for (int i = 0; i < 3; i++) {
			final int filter = i + 1;
			host.add(new Button(titles[i] + " ▾", () -> {
				expandedFilter = expandedFilter == filter ? 0 : filter;
				resetScroll();
				host.rebuild();
			}).selected(expandedFilter == filter)).bounds(x + i * (cw + GAP), y, cw, CHIP_H);
		}
		y += CHIP_H + GAP;
		for (int i = 0; i < 2; i++) {
			final boolean compatible = i == 0;
			host.add(new Button(compatible ? "Minecraft " + ArcticClient.platform().minecraftVersion() : "All versions", () -> {
				THIS_VERSION[k] = compatible;
				resetScroll();
				host.rebuild();
			}).selected(THIS_VERSION[k] == compatible)).bounds(x + i * (CHIP_W + GAP), y, CHIP_W, CHIP_H);
		}
		listTop = y + CHIP_H + GAP;
		Scroll scroll = currentScroll = host.scroll(x, listTop, x + w, bottom);
		int ry = listTop;
		if (expandedFilter != 0) {
			final int filter = expandedFilter;
			String[] choices = filter == 1 ? SORT_LABELS : filter == 2 ? CATEGORIES[k] : DETAILS[k];
			int selected = filter == 1 ? SORT[k] : filter == 2 ? CATEGORY[k] : DETAIL[k];
			for (int i = 0; i < choices.length; i++) {
				final int choice = i;
				scroll.add(host.add(new Button(filter == 1 ? choices[i] : label(choices[i]), () -> {
					if (filter == 1) SORT[k] = choice;
					else if (filter == 2) CATEGORY[k] = choice;
					else DETAIL[k] = choice;
					expandedFilter = 0;
					resetScroll();
					host.rebuild();
				}).selected(selected == i).bounds(x + (i % 3) * (cw + GAP), ry + (i / 3) * (CHIP_H + GAP), cw, CHIP_H)));
			}
			ry += ((choices.length + 2) / 3) * (CHIP_H + GAP) + GAP;
		}
		final PackIcons icons = ArcticClient.packIcons();
		List<PackBrowser.Pack> list = browser.results();
		empty = list.isEmpty();
		int rowW = w - 8;
		for (final PackBrowser.Pack pack : list) {
			String by = "by " + pack.author + " · " + downloads(pack.downloads) + " · " + pack.description;
			if (!pack.compatible) {
				by = "Not listed for Minecraft " + ArcticClient.platform().minecraftVersion() + " · " + by;
			}
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
			return new Button(pack.compatible ? "Install" : "Other version", () -> {
				ArcticClient.shaders().clearStatus();
				browser.install(pack);
			})
					.primary().enabled(browser.busy() == null && pack.compatible);
		}
		if (shader) {
			ShaderPacks shaders = ArcticClient.shaders();
			boolean on = file.equals(shaders.active());
			return new Button(!shaders.available() ? "Installed" : on ? "Turn off" : "Turn on", () -> shaders.select(on ? null : file))
					.selected(on).enabled(shaders.available() && !shaders.changing());
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

	private static String label(String value) {
		if (value.isEmpty()) {
			return "All";
		}
		return Character.isDigit(value.charAt(0)) ? value : Character.toUpperCase(value.charAt(0)) + value.substring(1).replace('-', ' ');
	}

	private static void openFolder(java.io.File folder) {
		if (folder == null) {
			return;
		}
		com.arcticlauncher.client.feature.FolderOpener.open(folder);
	}

	@Override
	public void tick(Host host) {
		if (view == 1 && typedAt > 0 && System.currentTimeMillis() - typedAt > SEARCH_DELAY_MS) {
			typedAt = 0;
			resetScroll();
			browser().search(query, filters());
		}
		if (view == 0 && System.currentTimeMillis() - refreshedAt > 2000) {
			refreshedAt = System.currentTimeMillis();
			host.rebuild();
		}
	}

	@Override
	public void draw(Gfx g, Style s, int mx, int my) {
		if (!empty || expandedFilter != 0) {
			return;
		}
		String text = view == 0 ? "No packs yet. Browse Modrinth, or drop zips in the folder."
				: browser().searching() ? "Searching…" : "No packs match. Adjust the filters or refresh.";
		Draw.centered(g, text, x + w / 2, listTop + 30, s.muted, false);
	}

	@Override
	public String state() {
		PackBrowser b = browser();
		StringBuilder out = new StringBuilder().append(shader).append('/').append(view).append('/').append(b.busy()).append('/').append(b.searching())
				.append('/').append(b.hasMore()).append('/').append(applying).append('/');
		if (shader) {
			out.append(ArcticClient.shaders().active()).append('/').append(ArcticClient.shaders().changing()).append('/');
		}
		for (PackBrowser.Pack p : b.results()) {
			out.append(p.id).append(',');
		}
		return out.toString();
	}
}
