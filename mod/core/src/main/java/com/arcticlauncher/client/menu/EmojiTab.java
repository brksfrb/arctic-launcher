package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.EmoteAtlas;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;

/** The same approved pictures as chat; copying preserves the readable shortcode. */
final class EmojiTab implements MenuTab {
	public String title() { return "Chat emotes"; }
	public String hint() { return "Click a picture to copy its shortcode, then paste into chat."; }
	public void build(Host host, int x, int top, int w, int bottom) {
		final ClientConfig c = ArcticClient.config();
		Form f = new Form(host, x, top, w, bottom);
		f.section("Typing");
		f.toggle("Chat button", "The :) button opens the picture picker", () -> c.emojiButton, on -> c.emojiButton = on);
		f.toggle("Chat emotes", "Show pictures for shortcodes; send readable :name: text", () -> c.chatEmoji, on -> c.chatEmoji = on);
		f.note("Twemoji by Twitter, CC-BY 4.0. Arctic mark and ice crystal by Arctic.");
		int perRow = Math.max(1, f.w / 25), column = 0, y = 0;
		String group = "";
		for (final EmoteAtlas.Emote emote : EmoteAtlas.all()) {
			if (!group.equals(emote.group)) { f.section(emote.group); column = 0; group = emote.group; }
			if (column == 0) y = f.row(25);
			f.put(new Button("", () -> {
				ArcticClient.platform().setClipboard(emote.shortcode());
				com.arcticlauncher.client.notice.Notices.post("Copied " + emote.shortcode(), "Paste into chat with Ctrl+V");
			}) {
				@Override protected void draw(Gfx g, Style s, int mx, int my, float dt) {
					super.draw(g, s, mx, my, dt);
					emote.draw(g, x + 2, y + 2, 18);
				}
			}, f.x + column * 25, y, 22, 22);
			column = (column + 1) % perRow;
		}
		f.done();
	}
}
