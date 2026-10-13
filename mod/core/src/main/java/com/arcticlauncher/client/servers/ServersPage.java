package com.arcticlauncher.client.servers;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.TextField;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Arctic's multiplayer list, in place of the game's: your servers with their icons, messages
 * and pings; pick one up and drag it to reorder (the others make room as it moves); LAN worlds
 * below. Add, edit, delete and direct connect work as in the game, in the same servers.dat.
 * The Discover tab lists Arctic's public servers (the launcher's list), to join or add.
 */
public final class ServersPage extends Page {
	private static final int LIST_W = 340;
	private static final int ROW_H = 36;
	private static final int ROW_GAP = 3;
	private static final int ICON = 32;
	private static final int TOP = 34;
	private static final int BUTTONS_H = 52;
	private static final int BUTTON_H = 20;
	private static final int GAP = 4;
	/** Pixels the mouse moves before a press becomes a drag. */
	private static final int DRAG_START = 4;
	private static final long DOUBLE_CLICK_NS = 350_000_000L;
	/** How fast rows slide into their places (per second, a share of the way). */
	private static final float SLIDE = 18f;
	/** While dragging near the list's top or bottom edge, it scrolls this fast (pixels per second). */
	private static final float EDGE_SCROLL = 240f;
	private static final int EDGE = 18;

	private enum Mode { LIST, ADD, EDIT, DIRECT }

	private final Platform platform = ArcticClient.platform();
	private final Servers servers;
	private final LanScan lan = new LanScan();
	private List<Servers.Entry> entries;
	/** Where each row is drawn now (it slides toward its place), by entry. */
	private final Map<Servers.Entry, Float> shownAt = new HashMap<Servers.Entry, Float>();
	private int selected = -1;
	/** The selected LAN world (its address), when one is. */
	private String selectedLan;
	private float scroll;
	private Mode mode = Mode.LIST;
	private TextField nameField;
	private TextField addressField;

	// The row being pressed or dragged.
	private int pressed = -1;
	private double pressX;
	private double pressY;
	private double grabDy;
	private boolean dragging;
	private double mouseY;
	private long lastClickAt;
	private int lastClickRow = -1;
	private long lastFrame;
	/** On the Discover tab: Arctic's public servers (null while loading). */
	private boolean discover;
	private volatile List<Servers.Entry> publicList;
	private volatile String publicError;
	private int selectedPublic = -1;

	public ServersPage() {
		servers = new Servers(platform, platform.configDir().getAbsoluteFile().getParentFile());
		entries = servers.load();
		servers.pingAll(entries);
		lan.start();
	}


	private static String tr(String key, String english) {
		return ArcticClient.platform().translate(key, english);
	}

	@Override
	public boolean ownBackground() {
		return !platform.inWorld();
	}

	@Override
	public void removed() {
		lan.stop();
		super.removed();
	}

	private int listLeft() {
		return width / 2 - LIST_W / 2;
	}

	private int listTop() {
		return TOP;
	}

	private int listBottom() {
		return height - BUTTONS_H - 6;
	}

	@Override
	protected void build() {
		if (mode != Mode.LIST) {
			buildForm();
			return;
		}
		int tab = 96;
		add(new Button("My Servers", () -> tab(false)).selected(!discover)).bounds(width / 2 - tab - GAP / 2, 8, tab, 18);
		add(new Button("Discover", () -> tab(true)).selected(discover)).bounds(width / 2 + GAP / 2, 8, tab, 18);
		if (discover) {
			buildDiscover();
			return;
		}
		int bw = (LIST_W - GAP * 2) / 3;
		int left = listLeft();
		int y = height - BUTTONS_H;
		boolean any = selected >= 0 || selectedLan != null;
		add(new Button(tr("selectServer.select", "Join Server"), this::joinSelected).primary().enabled(any)).bounds(left, y, bw, BUTTON_H);
		add(new Button(tr("selectServer.direct", "Direct Connection"), () -> open(Mode.DIRECT))).bounds(left + bw + GAP, y, bw, BUTTON_H);
		add(new Button(tr("selectServer.add", "Add Server"), () -> open(Mode.ADD))).bounds(left + (bw + GAP) * 2, y, LIST_W - (bw + GAP) * 2, BUTTON_H);
		y += BUTTON_H + GAP;
		int qw = (LIST_W - GAP * 3) / 4;
		add(new Button(tr("selectServer.edit", "Edit"), () -> open(Mode.EDIT)).enabled(selected >= 0)).bounds(left, y, qw, BUTTON_H);
		add(new Button(tr("selectServer.delete", "Delete"), this::deleteSelected).confirm("Delete it?").enabled(selected >= 0))
				.bounds(left + qw + GAP, y, qw, BUTTON_H);
		add(new Button(tr("selectServer.refresh", "Refresh"), () -> servers.pingAll(entries))).bounds(left + (qw + GAP) * 2, y, qw, BUTTON_H);
		add(new Button(tr("gui.back", "Back"), () -> platform.openPage(ArcticClient.titleMenu())))
				.bounds(left + (qw + GAP) * 3, y, LIST_W - (qw + GAP) * 3, BUTTON_H);
	}

	private void buildDiscover() {
		int left = listLeft();
		int y = height - BUTTONS_H;
		int bw = (LIST_W - GAP) / 2;
		boolean any = selectedPublic >= 0 && publicList != null && selectedPublic < publicList.size();
		add(new Button(tr("selectServer.select", "Join Server"), () -> {
			if (publicList != null && selectedPublic >= 0) {
				platform.connectTo(publicList.get(selectedPublic).address);
			}
		}).primary().enabled(any)).bounds(left, y, bw, BUTTON_H);
		add(new Button("Add to My Servers", () -> {
			Servers.Entry e = publicList.get(selectedPublic);
			if (servers.add(e.name, e.address)) {
				reload();
				servers.ping(e.address);
			}
		}).enabled(any)).bounds(left + bw + GAP, y, LIST_W - bw - GAP, BUTTON_H);
		y += BUTTON_H + GAP;
		add(new Button(tr("selectServer.refresh", "Refresh"), this::loadPublic)).bounds(left, y, bw, BUTTON_H);
		add(new Button(tr("gui.back", "Back"), () -> platform.openPage(ArcticClient.titleMenu())))
				.bounds(left + bw + GAP, y, LIST_W - bw - GAP, BUTTON_H);
	}

	private void tab(boolean toDiscover) {
		if (discover == toDiscover) {
			return;
		}
		discover = toDiscover;
		scroll = 0;
		if (discover && publicList == null) {
			loadPublic();
		}
		rebuild();
	}

	/** Arctic's public list, in the background; each server is asked how it is, too. */
	private void loadPublic() {
		publicList = null;
		publicError = null;
		selectedPublic = -1;
		Thread t = new Thread(() -> {
			try {
				String json = com.arcticlauncher.client.looks.Http.getText(ArcticClient.looks().baseUrl() + "/v1/servers", null);
				List<Servers.Entry> list = new java.util.ArrayList<Servers.Entry>();
				for (com.google.gson.JsonElement e : new com.google.gson.JsonParser().parse(json).getAsJsonObject().getAsJsonArray("servers")) {
					com.google.gson.JsonObject o = e.getAsJsonObject();
					list.add(new Servers.Entry(o.get("name").getAsString(), o.get("address").getAsString()));
				}
				servers.pingAll(list);
				publicList = list;
			} catch (Exception e) {
				publicError = "Couldn't load the server list";
			}
		}, "arctic-public-servers");
		t.setDaemon(true);
		t.start();
	}

	/** Add, edit or direct connect: a small form over the list. */
	private void buildForm() {
		int w = 240;
		int x = width / 2 - w / 2;
		int y = height / 2 - 40;
		boolean direct = mode == Mode.DIRECT;
		Servers.Entry editing = mode == Mode.EDIT && selected >= 0 ? entries.get(selected) : null;
		if (!direct) {
			nameField = add(new TextField(tr("addServer.enterName", "Server Name"), 64, TextField.ANY));
			nameField.bounds(x, y, w, BUTTON_H);
			if (editing != null) {
				nameField.text(editing.name);
			}
			y += BUTTON_H + GAP + 10;
		}
		addressField = add(new TextField(tr("addServer.enterIp", "Server Address"), 256, TextField.ANY));
		addressField.bounds(x, y, w, BUTTON_H);
		addressField.onEnter(this::submit);
		if (editing != null) {
			addressField.text(editing.address);
		} else if (direct && ArcticClient.config().lastDirect != null) {
			addressField.text(ArcticClient.config().lastDirect);
		}
		focus(direct ? addressField : nameField);
		y += BUTTON_H + GAP * 2;
		int bw = (w - GAP) / 2;
		String ok = direct ? tr("selectServer.select", "Join Server") : tr("gui.done", "Done");
		add(new Button(ok, this::submit).primary()).bounds(x, y, bw, BUTTON_H);
		add(new Button(tr("gui.cancel", "Cancel"), () -> open(Mode.LIST))).bounds(x + bw + GAP, y, w - bw - GAP, BUTTON_H);
	}

	private void open(Mode m) {
		mode = m;
		rebuild();
	}

	private void submit() {
		String address = addressField.text().trim();
		if (address.isEmpty()) {
			return;
		}
		if (mode == Mode.DIRECT) {
			ArcticClient.config().lastDirect = address;
			ArcticClient.saveConfig();
			platform.connectTo(address);
			return;
		}
		String name = nameField.text().trim().isEmpty() ? tr("selectServer.defaultName", "Minecraft Server") : nameField.text().trim();
		if (mode == Mode.EDIT && selected >= 0) {
			servers.edit(selected, name, address);
		} else {
			servers.add(name, address);
		}
		reload();
		servers.ping(address);
		open(Mode.LIST);
	}

	private void reload() {
		entries = servers.load();
		if (selected >= entries.size()) {
			selected = entries.size() - 1;
		}
	}

	private void deleteSelected() {
		if (selected < 0) {
			return;
		}
		servers.remove(selected);
		selected = -1;
		reload();
		rebuild();
	}

	private void joinSelected() {
		if (selectedLan != null) {
			platform.connectTo(selectedLan);
		} else if (selected >= 0) {
			servers.join(selected);
		}
	}

	private void select(int row, String lanAddress) {
		boolean changed = row != selected || (lanAddress == null ? selectedLan != null : !lanAddress.equals(selectedLan));
		selected = row;
		selectedLan = lanAddress;
		if (changed) {
			rebuild();
		}
	}

	// ---- Layout --------------------------------------------------------------------------

	/** Where saved row {@code i} belongs (top edge, unscrolled), with the dragged one's gap. */
	private float slotTop(int i) {
		return i * (ROW_H + ROW_GAP);
	}

	/** The index the dragged row would take if let go now. */
	private int dropIndex() {
		float top = (float) (mouseY - grabDy - listTop() + scroll);
		int index = Math.round(top / (ROW_H + ROW_GAP));
		return Math.max(0, Math.min(entries.size() - 1, index));
	}

	/** Each row's place, the dragged one's taken out and its gap moved to where it'd drop. */
	private float targetTop(int i) {
		if (!dragging) {
			return slotTop(i);
		}
		int drop = dropIndex();
		int order = i;
		if (i > pressed && i <= drop) {
			order = i - 1;
		} else if (i < pressed && i >= drop) {
			order = i + 1;
		}
		return slotTop(order);
	}

	private float contentHeight() {
		int lanRows = lan.worlds().size();
		float h = entries.size() * (ROW_H + ROW_GAP);
		return lanRows == 0 ? h : h + 14 + lanRows * (ROW_H + ROW_GAP);
	}

	private void clampScroll() {
		float max = Math.max(0, contentHeight() - (listBottom() - listTop()));
		scroll = Math.max(0, Math.min(max, scroll));
	}

	// ---- Drawing -------------------------------------------------------------------------

	@Override
	protected void drawAbove(Gfx g, Style s, int mx, int my) {
		drawTip(g, s);
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		tip = null;
		tipX = mx;
		tipY = my;
		long now = System.nanoTime();
		float dt = lastFrame == 0 ? 0f : Math.min(0.1f, (now - lastFrame) / 1e9f);
		lastFrame = now;
		int left = listLeft();
		int top = listTop();
		int bottom = listBottom();
		if (discover) {
			drawDiscover(g, s, left, top, bottom, mx, my);
			return;
		}
		if (dragging) {
			mouseY = my;
			// Near an edge: the list scrolls so the row can go anywhere.
			if (my < top + EDGE) {
				scroll -= EDGE_SCROLL * dt;
			} else if (my > bottom - EDGE) {
				scroll += EDGE_SCROLL * dt;
			}
		}
		clampScroll();
		Skin.panel(g, s, left - 4, top - 4, left + LIST_W + 4, bottom + 4);
		g.scissor(left - 3, top - 3, left + LIST_W + 3, bottom + 3);
		for (int i = 0; i < entries.size(); i++) {
			Servers.Entry e = entries.get(i);
			float target = targetTop(i);
			Float at = shownAt.get(e);
			float y = at == null ? target : at + (target - at) * Math.min(1f, dt * SLIDE);
			shownAt.put(e, y);
			if (dragging && i == pressed) {
				continue;
			}
			int ry = Math.round(top + y - scroll);
			if (ry + ROW_H < top - 4 || ry > bottom + 4) {
				continue;
			}
			boolean over = mode == Mode.LIST && !dragging && mx >= left && mx < left + LIST_W && my >= ry && my < ry + ROW_H
					&& my >= top && my < bottom;
			row(g, s, left, ry, e.name, e.address, i == selected, over);
		}
		drawLan(g, s, left, top, bottom, mx, my);
		g.endScissor();
		if (dragging && pressed >= 0 && pressed < entries.size()) {
			// The row in your hand, a little lifted.
			Servers.Entry e = entries.get(pressed);
			int ry = (int) Math.round(mouseY - grabDy);
			Draw.round(g, left + 2, ry + 3, left + LIST_W + 2, ry + ROW_H + 3, 4, 0x55000000);
			row(g, s, left, ry, e.name, e.address, true, true);
			Draw.outline(g, left, ry, left + LIST_W, ry + ROW_H, 4, s.accent);
		}
		if (entries.isEmpty() && lan.worlds().isEmpty()) {
			Draw.centered(g, "No servers yet: Add Server or Direct Connection", width / 2, (top + bottom) / 2 - 4, s.muted, false);
		}
		if (mode != Mode.LIST) {
			g.fill(0, 0, width, height, 0x88000000);
			Skin.panel(g, s, width / 2 - 130, height / 2 - 60, width / 2 + 130, height / 2 + 50);
			String title = mode == Mode.DIRECT ? tr("selectServer.direct", "Direct Connection")
					: mode == Mode.EDIT ? tr("selectServer.edit", "Edit") : tr("selectServer.add", "Add Server");
			Draw.centered(g, title, width / 2, height / 2 - 54, s.text, false);
		}
	}

	private void drawDiscover(Gfx g, Style s, int left, int top, int bottom, int mx, int my) {
		List<Servers.Entry> list = publicList;
		float max = list == null ? 0 : Math.max(0, list.size() * (ROW_H + ROW_GAP) - (bottom - top));
		scroll = Math.max(0, Math.min(max, scroll));
		Skin.panel(g, s, left - 4, top - 4, left + LIST_W + 4, bottom + 4);
		if (list == null) {
			Draw.centered(g, publicError != null ? publicError : "Loading…", width / 2, (top + bottom) / 2 - 4, s.muted, false);
			return;
		}
		g.scissor(left - 3, top - 3, left + LIST_W + 3, bottom + 3);
		for (int i = 0; i < list.size(); i++) {
			int ry = Math.round(top + slotTop(i) - scroll);
			if (ry + ROW_H < top - 4 || ry > bottom + 4) {
				continue;
			}
			boolean over = mx >= left && mx < left + LIST_W && my >= ry && my < ry + ROW_H && my >= top && my < bottom;
			row(g, s, left, ry, list.get(i).name, list.get(i).address, i == selectedPublic, over);
		}
		g.endScissor();
	}

	private void drawLan(Gfx g, Style s, int left, int top, int bottom, int mx, int my) {
		List<LanScan.World> worlds = lan.worlds();
		if (worlds.isEmpty()) {
			return;
		}
		float y = entries.size() * (ROW_H + ROW_GAP);
		g.text(tr("lanServer.title", "LAN World"), left + 2, Math.round(top + y - scroll + 2), s.muted, false);
		y += 14;
		for (LanScan.World w : worlds) {
			int ry = Math.round(top + y - scroll);
			boolean over = mode == Mode.LIST && !dragging && mx >= left && mx < left + LIST_W && my >= ry && my < ry + ROW_H;
			lanRow(g, s, left, ry, w, w.address.equals(selectedLan), over);
			y += ROW_H + ROW_GAP;
		}
	}

	private void background(Gfx g, Style s, int x, int y, boolean selectedRow, boolean over) {
		int fill = selectedRow ? Draw.alpha(s.accent, 0.22f) : over ? s.buttonHover : Draw.alpha(s.button, 0.6f);
		Draw.round(g, x, y, x + LIST_W, y + ROW_H, 4, fill);
		if (selectedRow) {
			Draw.outline(g, x, y, x + LIST_W, y + ROW_H, 4, Draw.alpha(s.accent, 0.8f));
		}
	}

	/** What the mouse is over in a row (the ping bars, the player count, the version), shown above it all. */
	private List<String> tip;
	private int tipX;
	private int tipY;

	private void row(Gfx g, Style s, int x, int y, String name, String address, boolean selectedRow, boolean over) {
		background(g, s, x, y, selectedRow, over);
		Servers.Answer a = servers.answer(address);
		int ix = x + 2;
		int iy = y + 2;
		if (a != null && a.icon != null) {
			g.texture(a.icon, ix, iy, ICON, ICON, 0, 0, 64, 64, 64, 64);
		} else {
			Draw.round(g, ix, iy, ix + ICON, iy + ICON, 3, s.field);
			Draw.centered(g, "?", ix + ICON / 2, iy + ICON / 2 - 4, s.muted, false);
		}
		int tx = x + ICON + 8;
		int right = x + LIST_W - 6;
		boolean up = a != null && a.status != null;
		boolean down = a != null && a.error != null;
		// Top right: how fast it answered (or that it didn't), always shown, then the bars.
		String speed = up ? (a.status.ping < 0 ? "?" : a.status.ping + " ms") : down ? "Offline" : "...";
		int speedColor = up ? pingColor(a.status.ping) : down ? RED : s.muted;
		int barsX = right - BARS_W;
		int speedW = g.textWidth(speed);
		g.text(speed, barsX - 4 - speedW, y + 4, speedColor, false);
		pingBars(g, s, barsX, y + 3, a);
		// Under it: who's on, then the version it advertises (red when it isn't this game's).
		String players = up ? (a.status.online > 0 ? "§a" : "§7") + group(a.status.online) + "§8/§7" + group(a.status.max) : "";
		String version = up ? a.status.version : "";
		boolean incompatible = up && a.status.protocol > 0 && platform.protocol() > 0 && a.status.protocol != platform.protocol();
		int playersW = g.textWidth(players);
		int versionW = Math.min(g.textWidth(version), 90);
		g.text(players, right - playersW, y + 14, s.text, false);
		if (!version.isEmpty()) {
			g.text(Draw.fit(g, version, versionW), right - versionW, y + 24, incompatible ? RED : 0xFF7D93AE, false);
		}
		int column = Math.max(playersW, versionW) + 8;
		g.text(Draw.fit(g, name, barsX - 8 - speedW - tx), tx, y + 4, s.text, false);
		if (over && tipY >= y + 2 && tipY <= y + 13) {
			if (tipX >= barsX - 4 - speedW && tipX <= right) {
				tip = speedTip(a, incompatible);
			}
		} else if (over && up && tipY > y + 13 && tipY <= y + 23 && tipX >= right - playersW && tipX <= right) {
			tip = playersTip(a);
		}
		if (a == null || !a.done()) {
			g.text("Pinging…", tx, y + 15, s.muted, false);
			return;
		}
		if (down) {
			g.text("§c" + a.error, tx, y + 15, s.muted, false);
			return;
		}
		String[] lines = a.status.motd.split("\n", -1);
		for (int i = 0; i < Math.min(2, lines.length); i++) {
			g.text(Draw.fit(g, lines[i].replace("\r", ""), right - column - tx), tx, y + 15 + i * 10, 0xFFAAAAAA, false);
		}
	}

	private static final int RED = 0xFFF87171;
	/** Five bars, two wide with a gap: 14 across. */
	private static final int BARS_W = 14;

	/** Green when quick, through yellow and orange, to red. */
	private static int pingColor(long ms) {
		if (ms < 0) {
			return 0xFFAAAAAA;
		}
		return ms < 80 ? 0xFF4ADE80 : ms < 150 ? 0xFFA3E635 : ms < 300 ? 0xFFFACC15 : ms < 600 ? 0xFFFB923C : RED;
	}

	/** 12,345: thousands grouped, so big servers read at a glance. */
	private static String group(int n) {
		return String.format(java.util.Locale.ROOT, "%,d", n);
	}

	private List<String> speedTip(Servers.Answer a, boolean incompatible) {
		if (a == null || !a.done()) {
			return java.util.Collections.singletonList(tr("multiplayer.status.pinging", "Pinging..."));
		}
		if (a.error != null) {
			return java.util.Collections.singletonList("§c" + tr("multiplayer.status.no_connection", "(no connection)"));
		}
		List<String> lines = new java.util.ArrayList<String>();
		lines.add(a.status.ping < 0 ? "?" : a.status.ping + " ms");
		if (incompatible) {
			lines.add("§c" + tr("multiplayer.status.incompatible", "Incompatible version!") + " (" + a.status.version + ")");
		}
		return lines;
	}

	private static List<String> playersTip(Servers.Answer a) {
		List<String> lines = new java.util.ArrayList<String>(a.status.sample);
		if (lines.isEmpty()) {
			lines.add(a.status.online + " / " + a.status.max);
		} else if (a.status.online > lines.size()) {
			lines.add("§7... and " + (a.status.online - lines.size()) + " more");
		}
		return lines;
	}

	private void lanRow(Gfx g, Style s, int x, int y, LanScan.World w, boolean selectedRow, boolean over) {
		background(g, s, x, y, selectedRow, over);
		Draw.round(g, x + 2, y + 2, x + 2 + ICON, y + 2 + ICON, 3, s.field);
		Draw.centered(g, "LAN", x + 2 + ICON / 2, y + 14, s.accent, false);
		int tx = x + ICON + 8;
		g.text(Draw.fit(g, w.motd, LIST_W - ICON - 14), tx, y + 4, s.text, false);
		g.text(w.address, tx, y + 15, s.muted, false);
	}

	private void drawTip(Gfx g, Style s) {
		List<String> lines = tip;
		if (lines == null || lines.isEmpty()) {
			return;
		}
		int w = 0;
		for (String l : lines) {
			w = Math.max(w, g.textWidth(l));
		}
		int h = lines.size() * 10 + 6;
		int x = Math.min(tipX + 10, width - w - 10);
		int y = Math.max(4, Math.min(tipY - h - 4, height - h - 4));
		Draw.round(g, x - 4, y, x + w + 4, y + h, 3, 0xF0101828);
		Draw.outline(g, x - 4, y, x + w + 4, y + h, 3, Draw.alpha(s.accent, 0.5f));
		for (int i = 0; i < lines.size(); i++) {
			g.text(lines.get(i), x, y + 4 + i * 10, s.text, false);
		}
	}

	/** Five bars like the game's, by how fast the server answered: short and two wide. */
	private static void pingBars(Gfx g, Style s, int x, int y, Servers.Answer a) {
		int lit;
		int color;
		if (a == null || !a.done()) {
			lit = (int) (System.currentTimeMillis() / 150 % 5) + 1;
			color = s.muted;
		} else if (a.error != null) {
			lit = 0;
			color = RED;
		} else {
			long ms = a.status.ping;
			lit = ms < 0 ? 3 : ms < 150 ? 5 : ms < 300 ? 4 : ms < 600 ? 3 : ms < 1000 ? 2 : 1;
			color = pingColor(ms);
		}
		int bottom = y + 7;
		for (int i = 0; i < 5; i++) {
			int h = 3 + i;
			int bx = x + i * 3;
			g.fill(bx, bottom - h, bx + 2, bottom, i < lit ? color : 0x40FFFFFF);
		}
	}

	// ---- Input ---------------------------------------------------------------------------

	/** The saved row at the mouse (-1 for none). */
	private int rowAt(double mx, double my) {
		if (mx < listLeft() || mx >= listLeft() + LIST_W || my < listTop() || my >= listBottom()) {
			return -1;
		}
		float y = (float) (my - listTop() + scroll);
		for (int i = 0; i < entries.size(); i++) {
			float top = slotTop(i);
			if (y >= top && y < top + ROW_H) {
				return i;
			}
		}
		return -1;
	}

	/** The LAN world at the mouse, or null. */
	private String lanAt(double mx, double my) {
		if (mx < listLeft() || mx >= listLeft() + LIST_W || my < listTop() || my >= listBottom()) {
			return null;
		}
		float y = (float) (my - listTop() + scroll) - entries.size() * (ROW_H + ROW_GAP) - 14;
		List<LanScan.World> worlds = lan.worlds();
		for (int i = 0; i < worlds.size(); i++) {
			float top = i * (ROW_H + ROW_GAP);
			if (y >= top && y < top + ROW_H) {
				return worlds.get(i).address;
			}
		}
		return null;
	}

	@Override
	public boolean mouseClicked(double mx, double my, int button) {
		if (super.mouseClicked(mx, my, button) || mode != Mode.LIST || button != Keys.MOUSE_LEFT) {
			return true;
		}
		if (discover) {
			return clickPublic(mx, my);
		}
		int row = rowAt(mx, my);
		String lanWorld = row < 0 ? lanAt(mx, my) : null;
		if (row < 0 && lanWorld == null) {
			return false;
		}
		long now = System.nanoTime();
		boolean twice = row == lastClickRow && now - lastClickAt < DOUBLE_CLICK_NS;
		lastClickAt = now;
		lastClickRow = row;
		select(row, lanWorld);
		if (twice) {
			joinSelected();
			return true;
		}
		if (row >= 0) {
			pressed = row;
			pressX = mx;
			pressY = my;
			grabDy = my - (listTop() + slotTop(row) - scroll);
		}
		return true;
	}

	private boolean clickPublic(double mx, double my) {
		List<Servers.Entry> list = publicList;
		if (list == null || mx < listLeft() || mx >= listLeft() + LIST_W || my < listTop() || my >= listBottom()) {
			return false;
		}
		int row = (int) ((my - listTop() + scroll) / (ROW_H + ROW_GAP));
		if (row < 0 || row >= list.size() || (my - listTop() + scroll) - slotTop(row) >= ROW_H) {
			return false;
		}
		long now = System.nanoTime();
		boolean twice = row == lastClickRow && now - lastClickAt < DOUBLE_CLICK_NS;
		lastClickAt = now;
		lastClickRow = row;
		if (row != selectedPublic) {
			selectedPublic = row;
			rebuild();
		}
		if (twice) {
			platform.connectTo(list.get(row).address);
		}
		return true;
	}

	@Override
	public boolean mouseDragged(double mx, double my, int button) {
		if (pressed < 0) {
			return super.mouseDragged(mx, my, button);
		}
		mouseY = my;
		if (!dragging && (Math.abs(my - pressY) > DRAG_START || Math.abs(mx - pressX) > DRAG_START)) {
			dragging = entries.size() > 1;
		}
		return true;
	}

	@Override
	public boolean mouseReleased(double mx, double my, int button) {
		if (pressed < 0) {
			return super.mouseReleased(mx, my, button);
		}
		if (dragging) {
			int to = dropIndex();
			if (to != pressed) {
				if (servers.move(pressed, to)) {
					Servers.Entry moved = entries.remove(pressed);
					entries.add(to, moved);
					selected = to;
					// It settles into its new place from where it was let go.
					shownAt.put(moved, (float) (mouseY - grabDy - listTop() + scroll));
				}
				rebuild();
			}
		}
		pressed = -1;
		dragging = false;
		return true;
	}

	@Override
	public boolean mouseScrolled(double mx, double my, double amount) {
		if (mode != Mode.LIST) {
			return false;
		}
		scroll -= (float) amount * (ROW_H + ROW_GAP);
		if (!discover) {
			clampScroll();
		}
		return true;
	}

	@Override
	public boolean keyPressed(int key, int nativeKey) {
		if (mode != Mode.LIST && key == Keys.ESCAPE) {
			open(Mode.LIST);
			return true;
		}
		if (mode == Mode.LIST && key == Keys.ENTER && (selected >= 0 || selectedLan != null)) {
			joinSelected();
			return true;
		}
		if (mode == Mode.LIST && selected >= 0 && (key == Keys.UP || key == Keys.DOWN) && entries.size() > 1) {
			select(Math.max(0, Math.min(entries.size() - 1, selected + (key == Keys.UP ? -1 : 1))), null);
			return true;
		}
		return super.keyPressed(key, nativeKey);
	}

	@Override
	protected Page escapeTo() {
		return platform.inWorld() ? null : ArcticClient.titleMenu();
	}
}
