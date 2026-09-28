package com.arcticlauncher.client.menu;

import java.util.List;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.social.Social;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.together.Duel;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.TextField;
import com.arcticlauncher.client.voice.VoiceLink;
import com.google.gson.JsonObject;

/**
 * The Friends tab: invites, friends with who's online and where (Join,
 * Invite, Chat), and a chat with one friend (messages, screenshots, and
 * "Send my last screenshot").
 */
final class FriendsTab {
	private static final int ROW = 22;
	private static final int BTN_H = 16;
	private static final int DOT = 3;
	private static final int ONLINE = 0xFF86EFAC;
	private static final int OFFLINE = 0xFF64748B;
	private static final int THUMB_W = 96;
	private static final int LINE = 10;

	/** The friend whose chat is open (null = the list); kept while the game runs. */
	private static String chatWith;
	private static String chatName = "";
	private static int scroll;
	/** The voice players list is open instead of friends. */
	private static boolean voiceOpen;
	/** The kit the next duel uses (index into {@link Duel#KITS}). */
	private static int kit;
	private int duelY = -1;

	private TextField message;
	private int listTop;
	private int bottom;

	void build(final Host host, int x, int top, int w, int bottomY) {
		final Social social = ArcticClient.social();
		bottom = bottomY;
		if (social == null || !social.available()) {
			return;
		}
		if (chatWith != null) {
			buildChat(host, social, x, top, w);
			return;
		}
		final VoiceLink voice = ArcticClient.voice();
		if (voiceOpen && voice != null && voice.active()) {
			buildVoice(host, voice, x, top, w);
			return;
		}
		voiceOpen = false;
		int y = top;
		if (voice != null && voice.active()) {
			host.add(new Button("Players (" + voice.members().size() + ")", () -> {
				voiceOpen = true;
				scroll = 0;
				host.rebuild();
			})).bounds(x + w - 150, y, 76, BTN_H);
		}
		if (voice != null && voice.available()) {
			final com.arcticlauncher.client.config.ClientConfig c = ArcticClient.config();
			com.arcticlauncher.client.ui.KeyButton key = new com.arcticlauncher.client.ui.KeyButton(
					new com.arcticlauncher.client.ui.KeyButton.Binding() {
						@Override
						public String get() {
							return c.voiceKey;
						}

						@Override
						public void set(String k) {
							c.voiceKey = k;
							ArcticClient.saveConfig();
						}
					});
			host.add(key).bounds(x + w - 70, y, 70, BTN_H);
			host.listenKeys(key);
			y += ROW;
		}
		final Duel duel = ArcticClient.duel();
		duelY = -1;
		if (duel != null && duel.available()) {
			duelY = y;
			if (duel.hosting()) {
				host.add(new Button("New round", duel::newRound).primary()).bounds(x + w - 80, y, 80, BTN_H);
			} else {
				host.add(new Button("Kit: " + Duel.KITS[kit][1], () -> {
					kit = (kit + 1) % Duel.KITS.length;
					host.rebuild();
				})).bounds(x + w - 170, y, 84, BTN_H);
				host.add(new Button("Start a duel", () -> duel.start(Duel.KITS[kit][0], null, null)).primary())
						.bounds(x + w - 82, y, 82, BTN_H);
			}
			y += ROW;
		}
		for (final JsonObject inv : social.invites()) {
			final String target = inv.get("target").getAsString();
			final String kind = inv.get("kind").getAsString();
			final boolean server = "server".equals(kind);
			if (server) {
				host.add(new Button("Join", () -> join(social, target)).primary()).bounds(x + w - 90, y, 52, BTN_H);
			} else if ("together".equals(kind) && duel != null) {
				host.add(new Button("Join", () -> {
					social.dismissInvite(inv.get("id").getAsString());
					duel.join(target);
				}).primary()).bounds(x + w - 90, y, 52, BTN_H);
			}
			host.add(new Button("×", () -> {
				social.dismissInvite(inv.get("id").getAsString());
				host.rebuild();
			})).bounds(x + w - 34, y, 30, BTN_H);
			y += ROW;
		}
		listTop = y;
		List<Social.Friend> friends = social.friends();
		int rows = Math.max(1, (bottom - listTop) / ROW);
		scroll = Math.max(0, Math.min(scroll, Math.max(0, friends.size() - rows)));
		final String here = ArcticClient.platform().server();
		boolean onServer = here != null && !"Singleplayer".equals(here);
		for (int i = scroll; i < friends.size() && i < scroll + rows; i++) {
			final Social.Friend f = friends.get(i);
			int bx = x + w;
			String chat = f.unread > 0 ? "Chat (" + f.unread + ")" : "Chat";
			bx -= 58;
			host.add(new Button(chat, () -> {
				chatWith = f.id;
				chatName = f.name;
				social.openChat(f.id);
				host.rebuild();
			}).selected(f.unread > 0)).bounds(bx, y + 2, 56, BTN_H);
			if (f.server != null) {
				bx -= 44;
				host.add(new Button("Join", () -> join(social, f.server))).bounds(bx, y + 2, 42, BTN_H);
			}
			if (onServer && f.takesInvites) {
				bx -= 48;
				host.add(new Button("Invite", () -> social.invite(f, here))).bounds(bx, y + 2, 46, BTN_H);
			}
			if (duel != null && duel.available() && f.takesInvites && !duel.hosting()) {
				bx -= 44;
				host.add(new Button("Duel", () -> duel.start(Duel.KITS[kit][0], f.id, f.name))).bounds(bx, y + 2, 42, BTN_H);
			}
			y += ROW;
		}
		scrollButtons(host, friends.size(), rows, x, w);
	}

	private void buildChat(final Host host, final Social social, int x, int top, int w) {
		host.add(new Button("‹ Friends", () -> {
			chatWith = null;
			social.openChat(null);
			host.rebuild();
		})).bounds(x, top, 64, BTN_H);
		final java.io.File shot = social.lastScreenshot();
		if (shot != null) {
			host.add(new Button("Send last screenshot", () -> social.sendScreenshot(chatWith, shot)))
					.bounds(x + w - 120, top, 120, BTN_H);
		}
		int fieldY = bottom - 18;
		message = new TextField("Message " + chatName, 500, TextField.ANY);
		final Runnable send = () -> {
			String text = message.text().trim();
			if (!text.isEmpty()) {
				social.send(chatWith, text);
				message.text("");
			}
		};
		message.onEnter(send);
		host.add(message).bounds(x, fieldY, w - 50, 18);
		host.add(new Button("Send", send).primary()).bounds(x + w - 46, fieldY, 46, 18);
	}

	private void buildVoice(final Host host, final VoiceLink voice, int x, int top, int w) {
		host.add(new Button("‹ Friends", () -> {
			voiceOpen = false;
			scroll = 0;
			host.rebuild();
		})).bounds(x, top, 64, BTN_H);
		listTop = top + ROW;
		List<VoiceLink.Member> list = voice.members();
		int rows = Math.max(1, (bottom - listTop) / ROW);
		scroll = Math.max(0, Math.min(scroll, Math.max(0, list.size() - rows)));
		int y = listTop;
		for (int i = scroll; i < list.size() && i < scroll + rows; i++) {
			final VoiceLink.Member m = list.get(i);
			host.add(new Button(m.muted ? "Unmute" : "Mute", () -> {
				voice.toggleMute(m.uuid);
			}).selected(m.muted)).bounds(x + w - 58, y + 2, 56, BTN_H);
			y += ROW;
		}
		scrollButtons(host, list.size(), rows, x, w);
	}

	private void scrollButtons(final Host host, int count, int rows, int x, int w) {
		if (scroll > 0) {
			host.add(new Button("▲", () -> {
				scroll--;
				host.rebuild();
			})).bounds(x + w - 20, listTop - 1, 18, 12);
		}
		if (scroll + rows < count) {
			host.add(new Button("▼", () -> {
				scroll++;
				host.rebuild();
			})).bounds(x + w - 20, bottom - 12, 18, 12);
		}
	}

	private static void join(Social social, String server) {
		if (!social.join(server)) {
			com.arcticlauncher.client.notice.Notices.post("Join from the multiplayer list", server);
		}
	}

	void draw(Gfx g, Style s, int x, int top, int w) {
		Social social = ArcticClient.social();
		if (social == null || !social.available()) {
			g.text(Draw.fit(g, "Start the game from Arctic Launcher to see your friends here.", w, x, top), x, top, s.muted, false);
			return;
		}
		if (!social.status().isEmpty()) {
			g.text(Draw.fit(g, social.status(), w, x, bottom + 4), x, bottom + 4, s.accent, false);
		}
		if (chatWith != null) {
			drawChat(g, s, social, x, top + BTN_H + 6, w);
			return;
		}
		VoiceLink voice = ArcticClient.voice();
		if (voiceOpen && voice != null && voice.active()) {
			drawVoice(g, s, voice, x, top, w);
			return;
		}
		int y = top;
		if (voice != null && voice.available()) {
			String state = !voice.enabled() ? "Voice chat: off (turn it on in Arctic Launcher → Settings)"
					: !voice.active() ? "Voice chat: on (starts on a server)"
					: voice.listenOnly() ? "Voice chat: listening only (microphone blocked)" : "Voice chat: on · push to talk";
			g.text(Draw.fit(g, state, w - (voice.active() ? 156 : 80), x, y + 4), x, y + 4, voice.active() ? s.accent : s.muted, false);
			y += ROW;
		}
		Duel duel = ArcticClient.duel();
		if (duelY >= 0 && duel != null) {
			String line = duel.hosting() && duel.code() != null ? "Duel code: " + duel.code() : "Duel a friend: a flat arena with a kit";
			g.text(Draw.fit(g, line, w - 180, x, duelY + 4), x, duelY + 4, duel.hosting() ? s.accent : s.muted, false);
			y += ROW;
		}
		for (JsonObject inv : social.invites()) {
			String from = inv.getAsJsonObject("from").get("name").getAsString();
			String what = "together".equals(inv.get("kind").getAsString()) ? "a duel (or their world)" : inv.get("target").getAsString();
			g.text(Draw.fit(g, from + " invited you to " + what, w - 96, x, y + 4), x, y + 4, s.accent, false);
			y += ROW;
		}
		List<Social.Friend> friends = social.friends();
		if (friends.isEmpty()) {
			g.text(Draw.fit(g, "No friends yet: add them in Arctic Launcher → Friends.", w, x, y + 4), x, y + 4, s.muted, false);
			return;
		}
		int rows = Math.max(1, (bottom - listTop) / ROW);
		for (int i = scroll; i < friends.size() && i < scroll + rows; i++) {
			Social.Friend f = friends.get(i);
			int color = f.inGame ? ONLINE : f.online ? s.accent : OFFLINE;
			Draw.round(g, x, y + 7, x + DOT * 2, y + 7 + DOT * 2, DOT, color);
			String status = f.server != null ? "on " + f.server : f.inGame ? "in game" : f.online ? "online" : "offline";
			int room = w - (duel != null && duel.available() ? 200 : 160);
			g.text(Draw.fit(g, f.name, room, x + 10, y + 2), x + 10, y + 2, s.text, false);
			g.text(Draw.fit(g, status, room, x + 10, y + 11), x + 10, y + 11, s.muted, false);
			y += ROW;
		}
	}

	/** Players in voice on this server, with who's talking and who's muted. */
	private void drawVoice(Gfx g, Style s, VoiceLink voice, int x, int top, int w) {
		String title = voice.simpleVoiceChat() ? "Voice chat on this server (+ Simple Voice Chat)" : "Voice chat on this server";
		g.text(Draw.fit(g, title, w - 74, x + 70, top + 4), x + 70, top + 4, s.text, false);
		List<VoiceLink.Member> list = voice.members();
		if (list.isEmpty()) {
			g.text(Draw.fit(g, "Nobody else here uses Arctic voice yet.", w, x, listTop + 4), x, listTop + 4, s.muted, false);
			return;
		}
		int rows = Math.max(1, (bottom - listTop) / ROW);
		int y = listTop;
		for (int i = scroll; i < list.size() && i < scroll + rows; i++) {
			VoiceLink.Member m = list.get(i);
			int color = m.muted ? OFFLINE : m.speaking ? ONLINE : m.near ? s.accent : OFFLINE;
			Draw.round(g, x, y + 7, x + DOT * 2, y + 7 + DOT * 2, DOT, color);
			String status = m.muted ? "muted" : m.speaking ? "talking" : m.near ? "nearby" : "too far to hear";
			if (m.friend) {
				status += " · friend";
			} else if (m.svc) {
				status += " · Simple Voice Chat";
			}
			int room = w - 80;
			g.text(Draw.fit(g, m.name, room, x + 10, y + 2), x + 10, y + 2, s.text, false);
			g.text(Draw.fit(g, status, room, x + 10, y + 11), x + 10, y + 11, s.muted, false);
			y += ROW;
		}
	}

	/** Messages bottom-up above the field; screenshots as small pictures. */
	private void drawChat(Gfx g, Style s, Social social, int x, int top, int w) {
		g.text(Draw.fit(g, "Chat with " + chatName, w - 190, x + 70, top - BTN_H - 2), x + 70, top - BTN_H - 2, s.text, false);
		List<Social.Message> list = social.chat(chatWith);
		int y = bottom - 22;
		for (int i = list.size() - 1; i >= 0 && y > top; i--) {
			Social.Message m = list.get(i);
			boolean mine = !m.from.equals(chatWith);
			int color = mine ? s.accent : s.text;
			if (m.image != null) {
				int h = m.imageKey != null && m.imageW > 0 ? Math.round(m.imageH * THUMB_W / (float) m.imageW) : 54;
				y -= h + 2;
				if (y < top) {
					break;
				}
				int ix = mine ? x + w - THUMB_W : x;
				String hash = m.imageKey == null ? null : m.imageKey.substring(5);
				if (hash != null && ArcticClient.looks().isReady(hash)) {
					g.texture(m.imageKey, ix, y, THUMB_W, h, 0, 0, m.imageW, m.imageH, m.imageW, m.imageH);
				} else {
					Draw.round(g, ix, y, ix + THUMB_W, y + h, 3, 0x40FFFFFF);
				}
			}
			if (!m.text.isEmpty()) {
				List<String> lines = com.arcticlauncher.client.ui.Hints.wrap(g, m.text, w - 40);
				for (int l = lines.size() - 1; l >= 0 && y - LINE > top; l--) {
					y -= LINE;
					String line = lines.get(l);
					int lx = mine ? x + w - g.textWidth(line) : x;
					g.text(line, lx, y, color, false);
				}
			}
			y -= 4;
		}
		if (list.isEmpty()) {
			g.text("No messages yet. Say hi!", x, bottom - 34, s.muted, false);
		}
	}

	/** What the tab shows (the menu rebuilds when it changes). */
	static String state() {
		Social social = ArcticClient.social();
		if (social == null) {
			return "";
		}
		StringBuilder b = new StringBuilder().append(chatWith).append(social.available()).append(social.invites().size());
		VoiceLink voice = ArcticClient.voice();
		b.append(voiceOpen).append(voice != null && voice.active());
		Duel duel = ArcticClient.duel();
		b.append(duel != null && duel.hosting());
		if (voice != null) {
			for (VoiceLink.Member m : voice.members()) {
				b.append('|').append(m.uuid).append(m.muted);
			}
		}
		for (Social.Friend f : social.friends()) {
			b.append('|').append(f.id).append(f.online).append(f.inGame).append(f.server).append(f.unread);
		}
		b.append('|').append(social.lastScreenshot());
		return b.toString();
	}
}
