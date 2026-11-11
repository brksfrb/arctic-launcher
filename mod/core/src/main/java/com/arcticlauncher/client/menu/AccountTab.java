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

import com.arcticlauncher.client.Lang;
/**
 * The Account tab: switch accounts without restarting (through the
 * launcher), and the SOCKS5 proxy for server connections and Arctic's own
 * requests (the launcher sets the default; changes here win until the
 * launcher's setting changes again).
 */
final class AccountTab {
	private static final String[] PAGES = {"social.account_tab.accounts", "social.account_tab.proxy"};
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

	void build(final Host host, int x, int top, int w) {
		int pageW = (w - 4) / PAGES.length;
		for (int i = 0; i < PAGES.length; i++) {
			final int p = i;
			host.add(new Button(Lang.t(PAGES[i]), new Runnable() {
				@Override
				public void run() {
					page = p;
					host.rebuild();
				}
			}).selected(i == page)).bounds(x + i * (pageW + 4), top, pageW, 16);
		}
		if (page == 0) {
			buildAccounts(host, x, top + PAGE_BAR + 4, w);
			return;
		}
		buildProxy(host, x, top + PAGE_BAR + 4, w);
	}

	/** One row per launcher account; the current one can't be picked. */
	private void buildAccounts(final Host host, int x, int top, int w) {
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
		for (final AccountSwitcher.Entry entry : switcher.accounts()) {
			boolean isCurrent = entry.name.equalsIgnoreCase(current);
			Button b = new Button(isCurrent ? entry.name + "  (playing)" : entry.name + (entry.microsoft ? "" : "  (offline)"),
					new Runnable() {
						@Override
						public void run() {
							switcher.switchTo(entry);
						}
					});
			if (inWorld && !isCurrent) {
				// Switching means leaving; ask once, then rejoin as the new account.
				b.confirm(Lang.t("social.click_again_leave_switch_and_rejoin"));
			}
			b.enabled = !isCurrent && !switcher.busy();
			host.add(b).bounds(x, y, w, ACCOUNT_ROW - 4);
			y += ACCOUNT_ROW;
		}
		Button add = new Button(switcher.signingIn() ? Lang.t("social.signing_in") : Lang.t("social.add_account"), new Runnable() {
			@Override
			public void run() {
				switcher.addAccount();
			}
		});
		add.enabled = !switcher.signingIn();
		host.add(add).bounds(x, y, Math.min(w, ADD_W), ACCOUNT_ROW - 4);
		y += ACCOUNT_ROW;
		// An offline account: just a name (the launcher keeps it).
		final TextField offline = new TextField(Lang.t("social.offline_name"), MAX_NAME, NAME_CHARS);
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
		host.add(new Button(Lang.t("social.add_offline"), addOffline)).bounds(x + fieldW + GAP, y, ADD_W, FIELD_H);
		y += ACCOUNT_ROW;
		notesY = y + 4;
	}

	private void buildProxy(final Host host, int x, int top, int w) {
		if (draft == null) {
			draft = ArcticClient.config().proxy.copy();
		}
		int y = top;
		host.add(new Toggle(Lang.t("social.use_a_socks5_proxy"), Lang.t("social.servers_and_arctic_connect_through_it"), new Toggle.Binding() {
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
		int hostW = w - PORT_W - GAP;
		final TextField address = new TextField(Lang.t("social.address_like_127_0_0_1"), MAX_HOST, TextField.ANY).text(draft.host);
		final TextField port = new TextField(Lang.t("social.port"), MAX_PORT_DIGITS, TextField.DIGITS)
				.text(draft.port > 0 ? String.valueOf(draft.port) : "");
		final TextField user = new TextField(Lang.t("social.user_name_optional"), MAX_LOGIN, TextField.ANY).text(draft.username);
		final TextField pass = new TextField(Lang.t("social.password_optional"), MAX_LOGIN, TextField.ANY).password().text(draft.password);
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
		host.add(new Button(Lang.t("social.save"), new Runnable() {
			@Override
			public void run() {
				save();
				host.rebuild();
			}
		}).primary()).bounds(x, y, 70, 20);
		host.add(new Button(Lang.t("social.undo"), new Runnable() {
			@Override
			public void run() {
				draft = ArcticClient.config().proxy.copy();
				note = "";
				host.rebuild();
			}
		})).bounds(x + 76, y, 60, 20);
		notesY = y + 26;
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
			problem = Lang.t("social.a_password_needs_a_user_name_too");
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
			note = Lang.t("social.saved_new_connections_go_through_the_proxy");
		} else {
			note = wasOn ? Lang.t("social.proxy_off_restart_minecraft_to_reach_servers") : Lang.t("social.saved");
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
		g.text(Draw.fit(g, Lang.t("social.some_servers_block_proxies_and_vpns"), w, x, y), x, y, s.muted, false);
		g.text(Draw.fit(g, Lang.t("social.minecrafts_own_login_services_switch_at_the"), w, x, y + 11), x, y + 11, s.muted, false);
	}

	private void drawAccounts(Gfx g, Style s, int x, int w) {
		AccountSwitcher switcher = ArcticClient.accounts();
		int y = notesY;
		if (switcher == null || !switcher.available()) {
			g.text(Draw.fit(g, Lang.t("social.start_the_game_from_arctic_launcher_and_keep"), w, x, y), x, y, s.muted, false);
			g.text(Draw.fit(g, Lang.t("social.or_in_the_tray_to_switch_accounts_here"), w, x, y + 11), x, y + 11, s.muted, false);
			return;
		}
		if (!switcher.status().isEmpty()) {
			g.text(Draw.fit(g, switcher.status(), w, x, y), x, y, s.accent, false);
			y += 12;
		}
		if (ArcticClient.platform().inWorld()) {
			g.text(Draw.fit(g, Lang.t("social.leave_the_world_to_switch_accounts"), w, x, y), x, y, s.muted, false);
		}
	}
}
