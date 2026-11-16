package com.arcticlauncher.client.servers;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.MenuAction;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;
import java.util.ArrayList;
import java.util.List;

/**
 * In place of the game's disconnect screen when a server refused the join for missing or mismatched
 * mods: what the server named (and which of those this game has), one click to install them through
 * Arctic Launcher (Modrinth, hash-checked, approved in the launcher), then restart straight back into
 * the server. When the server didn't say which mods, it says so and points to linked instances.
 */
public final class MissingModsPage extends Page {
	private static final int BUTTON_W = 220;
	private static final int BUTTON_H = 20;
	private static final int GAP = 4;
	private static final int LINE_H = 10;
	private static final int MAX_LISTED = 10;
	private static final long POLL_MS = 1000;
	private static final int ERROR = 0xFFFF6B6B;
	private static final int WARN = 0xFFFFC857;
	private static final int OK = 0xFF7BD88F;

	private final Platform platform = ArcticClient.platform();
	private final MissingMods.Info info;
	private final String reason;
	private final String server;
	private final DisconnectedPage fallback;
	private final MissingModsLink link;
	/** Per mod: null missing, else the loaded version. */
	private final List<String> have = new ArrayList<String>();
	private volatile MissingModsLink.State state;
	private volatile boolean asking;
	private long polledAt;

	public MissingModsPage(MissingMods.Info info, String reason, String server, DisconnectedPage fallback) {
		this.info = info;
		this.reason = reason == null ? "" : reason;
		this.server = server;
		this.fallback = fallback;
		this.link = ArcticClient.missingModsLink();
		for (MissingMods.Entry e : info.mods) {
			have.add(platform.loadedModVersion(e.id));
		}
	}

	/** Which of the named mods this game lacks or runs another version of. */
	private int toInstall() {
		int n = 0;
		for (int i = 0; i < info.mods.size(); i++) {
			String version = have.get(i);
			String wanted = info.mods.get(i).version;
			if (version == null || (wanted != null && !version.equals(wanted) && !version.contains(wanted))) {
				n++;
			}
		}
		return n;
	}

	/** The server needs another loader than this game runs (Arctic can't change that here). */
	private boolean otherLoader() {
		if (info.loader == null) {
			return false;
		}
		String mine = platform.loaderId();
		return !info.loader.equals(mine) && !("quilt".equals(mine) && "fabric".equals(info.loader));
	}

	private boolean canInstall() {
		String s = state == null ? "idle" : state.state;
		return link != null && link.available() && !otherLoader() && toInstall() > 0 && !asking
				&& ("idle".equals(s) || "declined".equals(s) || "failed".equals(s) || "offline".equals(s));
	}

	private boolean installed() {
		return state != null && "installed".equals(state.state);
	}

	@Override
	public boolean ownBackground() {
		return true;
	}

	@Override
	protected Page escapeTo() {
		return new ServersPage();
	}

	@Override
	protected void build() {
		int left = width / 2 - BUTTON_W / 2;
		int y = height - 10 - 3 * (BUTTON_H + GAP);
		if (installed()) {
			add(new Button(Lang.t("servers.missing.restart_and_rejoin"), this::restart).primary()).bounds(left, y, BUTTON_W, BUTTON_H);
		} else if (canInstall()) {
			add(new Button(Lang.t("servers.missing.install_with_launcher"), this::install).primary()).bounds(left, y, BUTTON_W, BUTTON_H);
		} else if (fallback != null && fallback.canReconnect()) {
			add(new Button(Lang.t("servers.reconnect"), fallback::reconnect)).bounds(left, y, BUTTON_W, BUTTON_H);
		}
		y += BUTTON_H + GAP;
		add(new Button(platform.translate("gui.toMenu", Lang.t("servers.back_to_server_list")), () -> platform.openPage(new ServersPage()))).bounds(left, y, BUTTON_W, BUTTON_H);
		y += BUTTON_H + GAP;
		if (fallback != null) {
			add(new Button(Lang.t("servers.missing.show_full_message"), () -> platform.openPage(fallback))).bounds(left, y, BUTTON_W, BUTTON_H);
		}
	}

	private void install() {
		asking = true;
		rebuild();
		Thread t = new Thread(() -> {
			state = link.request(server, info, reason);
			asking = false;
			platform.runOnGameThread(this::rebuild);
		}, "arctic-missing-mods");
		t.setDaemon(true);
		t.start();
	}

	private void restart() {
		Thread t = new Thread(() -> {
			if (link.restart()) {
				platform.runOnGameThread(() -> platform.action(MenuAction.QUIT));
			}
		}, "arctic-missing-mods-restart");
		t.setDaemon(true);
		t.start();
	}

	@Override
	public void tick() {
		String s = state == null ? null : state.state;
		boolean waiting = "looking".equals(s) || "review".equals(s) || "installing".equals(s);
		long now = System.currentTimeMillis();
		if (!waiting || asking || now - polledAt < POLL_MS || link == null) {
			return;
		}
		polledAt = now;
		asking = true;
		Thread t = new Thread(() -> {
			MissingModsLink.State next = link.poll();
			MissingModsLink.State before = state;
			state = next;
			asking = false;
			if (before == null || !before.state.equals(next.state)) {
				platform.runOnGameThread(this::rebuild);
			}
		}, "arctic-missing-mods-poll");
		t.setDaemon(true);
		t.start();
	}

	/** What the launcher is doing, in words, and its color. */
	private String[] status(Style s) {
		if (link == null || !link.available()) {
			return new String[] {Lang.t("servers.missing.start_from_launcher"), String.valueOf(s.muted)};
		}
		if (state == null) {
			return null;
		}
		switch (state.state) {
			case "looking":
				return new String[] {Lang.t("servers.missing.looking_up"), String.valueOf(s.accent)};
			case "review":
				return new String[] {Lang.t("servers.missing.review_in_launcher"), String.valueOf(s.accent)};
			case "installing":
				return new String[] {Lang.t("servers.missing.installing"), String.valueOf(s.accent)};
			case "installed":
				return new String[] {Lang.t("servers.missing.installed", state.installed), String.valueOf(OK)};
			case "declined":
				return new String[] {Lang.t("servers.missing.declined"), String.valueOf(s.muted)};
			case "failed":
				return new String[] {state.message == null ? Lang.t("servers.missing.failed") : Lang.t("servers.missing.failed_because", state.message), String.valueOf(ERROR)};
			case "offline":
				return new String[] {Lang.t("servers.missing.launcher_not_answering"), String.valueOf(ERROR)};
			default:
				return null;
		}
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		int y = 16;
		int cx = width / 2;
		Draw.centered(g, Lang.t("servers.missing.title"), cx, y, s.text, true);
		y += LINE_H + 4;
		if (server != null) {
			Draw.centered(g, server, cx, y, s.muted, false);
			y += LINE_H + 6;
		}
		if (otherLoader()) {
			Draw.centered(g, Lang.t("servers.missing.needs_loader", loaderName(info.loader)), cx, y, WARN, false);
			y += LINE_H;
			Draw.centered(g, Lang.t("servers.missing.make_an_instance_with_it"), cx, y, s.muted, false);
			y += LINE_H + 6;
		}
		if (info.mods.isEmpty()) {
			Draw.centered(g, Lang.t("servers.missing.server_didnt_say_which"), cx, y, s.text, false);
			y += LINE_H;
			Draw.centered(g, Lang.t("servers.missing.ask_for_a_linked_instance"), cx, y, s.muted, false);
			y += LINE_H;
			Draw.centered(g, Lang.t("servers.missing.linked_instance_how"), cx, y, s.muted, false);
			y += LINE_H + 6;
			// What the server said, short.
			for (String line : firstLines(reason, 3)) {
				Draw.centered(g, line, cx, y, 0xFFAAAAAA, false);
				y += LINE_H;
			}
		} else {
			Draw.centered(g, Lang.t("servers.missing.server_needs"), cx, y, s.text, false);
			y += LINE_H + 4;
			int shown = Math.min(info.mods.size(), MAX_LISTED);
			for (int i = 0; i < shown; i++) {
				MissingMods.Entry e = info.mods.get(i);
				String version = have.get(i);
				String name = e.version == null ? e.id : e.id + " " + e.version;
				String what;
				int color;
				if (version == null) {
					what = Lang.t("servers.missing.missing");
					color = ERROR;
				} else if (e.version != null && !version.equals(e.version) && !version.contains(e.version)) {
					what = Lang.t("servers.missing.mismatched", version);
					color = WARN;
				} else {
					what = Lang.t("servers.missing.installed_here");
					color = OK;
				}
				Draw.centered(g, name + "  §r" + what, cx, y, color, false);
				y += LINE_H;
			}
			if (info.mods.size() > shown) {
				Draw.centered(g, Lang.t("servers.missing.and_more", info.mods.size() - shown), cx, y, s.muted, false);
				y += LINE_H;
			}
			if (toInstall() == 0 && !otherLoader()) {
				y += 4;
				Draw.centered(g, Lang.t("servers.missing.all_here_versions_differ"), cx, y, s.muted, false);
				y += LINE_H;
			}
		}
		String[] status = status(s);
		if (status != null) {
			y += 6;
			Draw.centered(g, status[0], cx, y, Integer.parseInt(status[1]), false);
			y += LINE_H;
			if (state != null && !state.notFound.isEmpty()) {
				Draw.centered(g, Lang.t("servers.missing.not_on_modrinth", join(state.notFound)), cx, y, WARN, false);
			}
		}
	}

	private static String loaderName(String id) {
		if ("neoforge".equals(id)) {
			return "NeoForge";
		}
		if ("forge".equals(id)) {
			return "Forge";
		}
		if ("quilt".equals(id)) {
			return "Quilt";
		}
		return "Fabric";
	}

	private static String join(List<String> items) {
		StringBuilder b = new StringBuilder();
		for (String item : items) {
			if (b.length() > 0) {
				b.append(", ");
			}
			b.append(item);
		}
		return b.toString();
	}

	private static List<String> firstLines(String text, int max) {
		List<String> out = new ArrayList<String>();
		for (String line : text.split("\n")) {
			if (out.size() >= max) {
				break;
			}
			String l = line.trim();
			if (!l.isEmpty()) {
				out.add(l.length() > 90 ? l.substring(0, 89) + "…" : l);
			}
		}
		return out;
	}
}
