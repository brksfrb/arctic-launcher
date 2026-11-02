package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.account.AccountSwitcher;
import com.arcticlauncher.client.config.ProxyConfig;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.TextField;
import com.arcticlauncher.client.ui.Toggle;
import com.arcticlauncher.client.ui.Widget;
import java.util.List;

/**
 * The Account tab: switch accounts without restarting (through the
 * launcher), and the SOCKS5 proxy for server connections and Arctic's own
 * requests (the launcher sets the default; changes here win until the
 * launcher's setting changes again).
 */
final class AccountTab {
	private static final String[] PAGES = {"Accounts", "Proxy"};
	private static final int PAGE_BAR = 20;
	private static final int ACCOUNT_ROW = 22;
	private static final int ADD_W = 120;
	/** Remembered while the game runs. */
	private static int page;

	private static final int FIELD_H = 18;
	private static final int MAX_NAME = 16;
	/** What a Minecraft name may hold. */
	private static final TextField.Filter NAME_CHARS = new TextField.Filter() {
		@Override
		public boolean accepts(int c) {
			return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_';
		}
	};
	private static final int GAP = 6;
	private static final int PORT_W = 44;
	private static final int MAX_HOST = 253;
	private static final int MAX_LOGIN = 255;
	private static final int MAX_PORT_DIGITS = 5;
	private static final int ROW = 24;
	private static final int WARN = 0xFFF87171;

	/** The form being edited; saved with the Save button. */
	private ProxyConfig draft;
	private String note = "";
	private boolean noteIsProblem;
	private int notesY;
	/** Which screenful of accounts shows (the list pages when it doesn't fit). */
	private int accountPage;
	private String pagerLabel;
	private int pagerY;
	/** Room kept under the list: the pager, Add account, the offline row and two note lines. */
	private static final int FOOTER = ROW + 3 * ACCOUNT_ROW + 28;

	void build(final Host host, int x, int top, int w, int bottom) {
		int pageW = (w - 4) / PAGES.length;
		for (int i = 0; i < PAGES.length; i++) {
			final int p = i;
			host.add(new Button(PAGES[i], new Runnable() {
				@Override
				public void run() {
					page = p;
					host.rebuild();
				}
			}).selected(i == page)).bounds(x + i * (pageW + 4), top, pageW, 16);
		}
		if (page == 0) {
			buildAccounts(host, x, top + PAGE_BAR + 4, w, bottom);
			return;
		}
		buildProxy(host, x, top + PAGE_BAR + 4, w);
	}

	/** One row per launcher account; the current one can't be picked. */
	private void buildAccounts(final Host host, int x, int top, int w, int bottom) {
		final AccountSwitcher switcher = ArcticClient.accounts();
		notesY = top;
		if (switcher == null || !switcher.available()) {
			return;
		}
		// Fresh every time the page opens (an account may have been added).
		switcher.refresh();
		String current = ArcticClient.platform().playerName();
		boolean inWorld = ArcticClient.platform().inWorld();
		int y = top;
		List<AccountSwitcher.Entry> all = switcher.accounts();
		int perPage = Math.max(1, (bottom - top - FOOTER) / ACCOUNT_ROW);
		int pages = Math.max(1, (all.size() + perPage - 1) / perPage);
		accountPage = Math.max(0, Math.min(accountPage, pages - 1));
		for (int i = accountPage * perPage; i < Math.min(all.size(), (accountPage + 1) * perPage); i++) {
			final AccountSwitcher.Entry entry = all.get(i);
			boolean isCurrent = isPlaying(entry, current);
			Button b = new Button(isCurrent ? entry.name + "  (playing)" : entry.name + (entry.microsoft ? "" : "  (offline)"),
					new Runnable() {
						@Override
						public void run() {
							switcher.switchTo(entry);
						}
					});
			if (inWorld && !isCurrent) {
				// Switching means leaving; ask once, then rejoin as the new account.
				b.confirm("Click again: leave, switch, and rejoin");
			}
			b.enabled = !isCurrent && !switcher.busy();
			host.add(b).bounds(x, y, w, ACCOUNT_ROW - 4);
			y += ACCOUNT_ROW;
		}
		if (pages > 1) {
			final int last = pages - 1;
			Button prev = new Button("<", new Runnable() {
				@Override
				public void run() {
					accountPage = Math.max(0, accountPage - 1);
					host.rebuild();
				}
			});
			prev.enabled = accountPage > 0;
			Button next = new Button(">", new Runnable() {
				@Override
				public void run() {
					accountPage = Math.min(last, accountPage + 1);
					host.rebuild();
				}
			});
			next.enabled = accountPage < last;
			host.add(prev).bounds(x, y, 40, ACCOUNT_ROW - 4);
			host.add(next).bounds(x + w - 40, y, 40, ACCOUNT_ROW - 4);
			pagerLabel = "Accounts " + (accountPage + 1) + " of " + pages;
			pagerY = y + 5;
			y += ACCOUNT_ROW;
		} else {
			pagerLabel = null;
		}
		Button add = new Button(switcher.signingIn() ? "Signing in..." : "Add account", new Runnable() {
			@Override
			public void run() {
				switcher.addAccount();
			}
		});
		add.enabled = !switcher.signingIn();
		host.add(add).bounds(x, y, Math.min(w, ADD_W), ACCOUNT_ROW - 4);
		y += ACCOUNT_ROW;
		// An offline account: just a name (the launcher keeps it).
		final TextField offline = new TextField("Offline name", MAX_NAME, NAME_CHARS);
		Runnable addOffline = new Runnable() {
			@Override
			public void run() {
				String name = offline.text().trim();
				if (!name.isEmpty()) {
					switcher.addOffline(name);
					offline.text("");
					host.rebuild();
				}
			}
		};
		offline.onEnter(addOffline);
		int fieldW = Math.max(60, Math.min(w, ADD_W + 60) - ADD_W - GAP);
		host.add(offline).bounds(x, y, fieldW, FIELD_H);
		host.add(new Button("Add offline", addOffline)).bounds(x + fieldW + GAP, y, ADD_W, FIELD_H);
		y += ACCOUNT_ROW;
		notesY = y + 4;
	}

	private void buildProxy(final Host host, int x, int top, int w) {
		if (draft == null) {
			draft = ArcticClient.config().proxy.copy();
		}
		int y = top;
		host.add(new Toggle("Use a proxy", "Servers and Arctic connect through it", new Toggle.Binding() {
			@Override
			public boolean get() {
				return draft.enabled;
			}

			@Override
			public void set(boolean on) {
				draft.enabled = on;
				note = "";
			}
		})).bounds(x, y, w, ROW);
		y += ROW + 6;
		int typeW = (w - GAP) / 2;
		host.add(new Button("SOCKS5", new Runnable() {
			@Override
			public void run() {
				draft.type = "socks5";
				note = "";
				host.rebuild();
			}
		}).selected(!draft.isHttp())).bounds(x, y, typeW, 20);
		host.add(new Button("HTTP", new Runnable() {
			@Override
			public void run() {
				draft.type = "http";
				note = "";
				host.rebuild();
			}
		}).selected(draft.isHttp())).bounds(x + typeW + GAP, y, w - typeW - GAP, 20);
		y += 20 + GAP;
		int hostW = w - PORT_W - GAP;
		final TextField address = new TextField("Address, like 127.0.0.1", MAX_HOST, TextField.ANY).text(draft.host);
		final TextField port = new TextField("Port", MAX_PORT_DIGITS, TextField.DIGITS)
				.text(draft.port > 0 ? String.valueOf(draft.port) : "");
		final TextField user = new TextField("User name (optional)", MAX_LOGIN, TextField.ANY).text(draft.username);
		final TextField pass = new TextField("Password (optional)", MAX_LOGIN, TextField.ANY).password().text(draft.password);
		Runnable sync = new Runnable() {
			@Override
			public void run() {
				draft.host = address.text().trim();
				draft.port = parsePort(port.text());
				draft.username = user.text();
				draft.password = pass.text();
				note = "";
			}
		};
		address.onChange(sync);
		port.onChange(sync);
		user.onChange(sync);
		pass.onChange(sync);
		host.add(address).bounds(x, y, hostW, FIELD_H);
		host.add(port).bounds(x + hostW + GAP, y, PORT_W, FIELD_H);
		y += FIELD_H + GAP;
		int half = (w - GAP) / 2;
		host.add(user).bounds(x, y, half, FIELD_H);
		host.add(pass).bounds(x + half + GAP, y, w - half - GAP, FIELD_H);
		y += FIELD_H + GAP + 2;
		host.add(new Button("Save", new Runnable() {
			@Override
			public void run() {
				save();
				host.rebuild();
			}
		}).primary()).bounds(x, y, 70, 20);
		host.add(new Button("Undo", new Runnable() {
			@Override
			public void run() {
				draft = ArcticClient.config().proxy.copy();
				note = "";
				host.rebuild();
			}
		})).bounds(x + 76, y, 60, 20);
		notesY = y + 26;
	}

	/**
	 * The account the game runs as: the name matches and so does the kind (an offline account is the
	 * one whose id is the name-derived one, so an offline and a Microsoft account of the same name
	 * aren't both "playing").
	 */
	private static boolean isPlaying(AccountSwitcher.Entry entry, String current) {
		if (!entry.name.equalsIgnoreCase(current)) {
			return false;
		}
		java.util.UUID id = ArcticClient.platform().playerId();
		if (id == null) {
			return true;
		}
		java.util.UUID offline = java.util.UUID.nameUUIDFromBytes(
				("OfflinePlayer:" + entry.name).getBytes(java.nio.charset.StandardCharsets.UTF_8));
		return id.equals(offline) != entry.microsoft;
	}

	private static int parsePort(String s) {
		try {
			return s.isEmpty() ? 0 : Integer.parseInt(s);
		} catch (NumberFormatException e) {
			return 0;
		}
	}

	private void save() {
		String problem = draft.enabled ? draft.problem() : null;
		if (problem == null && draft.username.isEmpty() && !draft.password.isEmpty()) {
			problem = "A password needs a user name too";
		}
		if (problem != null) {
			note = problem + ".";
			noteIsProblem = true;
			return;
		}
		boolean wasOn = ArcticClient.config().proxy.usable();
		ArcticClient.config().proxy = draft.copy();
		ArcticClient.saveConfig();
		ArcticClient.proxyChanged();
		noteIsProblem = false;
		if (draft.enabled) {
			note = "Saved. New connections go through the proxy.";
		} else {
			note = wasOn ? "Proxy off. Restart Minecraft to reach servers by name again." : "Saved.";
		}
	}

	void draw(Gfx g, Style s, int x, int w) {
		if (page == 0) {
			drawAccounts(g, s, x, w);
			return;
		}
		int y = notesY;
		if (!note.isEmpty()) {
			g.text(Draw.fit(g, note, w, x, y), x, y, noteIsProblem ? WARN : s.accent, false);
			y += 12;
		}
		g.text(Draw.fit(g, "Some servers block proxies and VPNs.", w, x, y), x, y, s.muted, false);
		g.text(Draw.fit(g, "Minecraft's own login services switch at the next start.", w, x, y + 11), x, y + 11, s.muted, false);
	}

	private void drawAccounts(Gfx g, Style s, int x, int w) {
		AccountSwitcher switcher = ArcticClient.accounts();
		if (pagerLabel != null) {
			g.text(pagerLabel, x + (w - g.textWidth(pagerLabel)) / 2, pagerY, s.muted, false);
		}
		int y = notesY;
		if (switcher == null || !switcher.available()) {
			g.text(Draw.fit(g, "Start the game from Arctic Launcher (and keep it open", w, x, y), x, y, s.muted, false);
			g.text(Draw.fit(g, "or in the tray) to switch accounts here.", w, x, y + 11), x, y + 11, s.muted, false);
			return;
		}
		if (!switcher.status().isEmpty()) {
			g.text(Draw.fit(g, switcher.status(), w, x, y), x, y, s.accent, false);
			y += 12;
		}
		if (ArcticClient.platform().inWorld()) {
			g.text(Draw.fit(g, "Leave the world to switch accounts.", w, x, y), x, y, s.muted, false);
		}
	}
}
