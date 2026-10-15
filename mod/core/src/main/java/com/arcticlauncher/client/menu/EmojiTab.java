package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.Emoji;
import com.arcticlauncher.client.ui.Button;
import java.util.Map;

/**
 * Symbols for chat: click one to copy it (paste into chat with Ctrl+V), or type its
 * shortcode (":heart:") and it's swapped when you send.
 */
final class EmojiTab implements MenuTab {
	private static final int CELL = 22;
	private static final int GAP = 3;

	@Override
	public String title() {
		return "Emoji and symbols";
	}

	@Override
	public String hint() {
		return "Click to copy, or type :heart: in chat.";
	}

	@Override
	public void build(Host host, int x, int top, int w, int bottom) {
		final ClientConfig c = ArcticClient.config();
		Form f = new Form(host, x, top, w, bottom);
		f.section("Typing");
		f.toggle("Shortcodes", "Type :heart: or :star: and it sends the symbol", () -> c.chatEmoji, on -> c.chatEmoji = on);
		StringBuilder sample = new StringBuilder();
		int n = 0;
		for (Map.Entry<String, String> e : Emoji.codes().entrySet()) {
			if (n++ >= 8) {
				break;
			}
			sample.append(':').append(e.getKey()).append(": ");
		}
		f.note("Some: " + sample.toString().trim());
		f.note("The full list is below, under each symbol's group.");
		int perRow = Math.max(1, f.w / (CELL + GAP));
		for (Map.Entry<String, String> group : Emoji.groups().entrySet()) {
			f.section(group.getKey());
			String[] symbols = group.getValue().split(" ");
			int row = 0;
			int topY = 0;
			for (int i = 0; i < symbols.length; i++) {
				if (i % perRow == 0) {
					topY = f.row(CELL + GAP);
					row++;
				}
				final String symbol = symbols[i];
				f.put(new Button(symbol, () -> {
					ArcticClient.platform().setClipboard(symbol);
					com.arcticlauncher.client.notice.Notices.post("Copied " + symbol, "Paste it into chat with Ctrl+V");
				}), f.x + (i % perRow) * (CELL + GAP), topY, CELL, CELL);
			}
		}
		f.section("Shortcodes");
		StringBuilder line = new StringBuilder();
		for (Map.Entry<String, String> e : Emoji.codes().entrySet()) {
			String item = ":" + e.getKey() + ": " + e.getValue() + "   ";
			if (line.length() + item.length() > 52) {
				f.note(line.toString().trim());
				line.setLength(0);
			}
			line.append(item);
		}
		if (line.length() > 0) {
			f.note(line.toString().trim());
		}
		f.done();
	}
}
