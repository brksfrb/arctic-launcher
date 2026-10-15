package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ChatFilter;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.ChatLog;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.util.ArrayList;
import java.util.List;

/**
 * A second chat on screen: the newest lines that match one of your saved filters (a name from
 * Chat history, say "Trades" with the words "buy, sell"), always visible, whatever else is
 * going on in the main chat.
 */
final class ChatFeed extends HudWidget {
	private static final int LINES = 6;
	private static final int LINE = 11;
	private static final int PAD = 4;
	private static final int WIDTH = 230;
	/** The lines are looked up again at most this often (milliseconds). */
	private static final long REFRESH_MS = 500;

	private List<ChatLog.Line> shown = new ArrayList<ChatLog.Line>();
	private long checkedAt;
	private long seen = -1;
	private String seenFilter = "";

	ChatFeed() {
		super("chatfeed", "Filtered chat", "A second chat: the newest lines that match a saved filter", false, Column.LEFT);
	}

	private static ChatFilter filter() {
		ClientConfig c = ArcticClient.config();
		if (c == null) {
			return null;
		}
		for (ChatFilter f : c.chatFilters) {
			if (f.name.equals(c.chatFeedFilter)) {
				return f;
			}
		}
		return null;
	}

	@Override
	public Object content(boolean preview) {
		// Redrawn whenever a line arrives or the filter changes; the lines themselves are cheap.
		ChatFilter f = filter();
		return preview ? "preview" : ArcticClient.chatLog().added() + "|" + (f == null ? "" : f.name + f.words);
	}

	@Override
	public int width(Gfx g) {
		return WIDTH;
	}

	@Override
	public int height() {
		return PAD + LINES * LINE;
	}

	private void refresh() {
		long now = System.currentTimeMillis();
		ChatFilter f = filter();
		String key = f == null ? "" : f.name + "|" + f.words;
		long added = ArcticClient.chatLog().added();
		if (added == seen && key.equals(seenFilter) && now - checkedAt < REFRESH_MS) {
			return;
		}
		checkedAt = now;
		seen = added;
		seenFilter = key;
		shown = f == null ? new ArrayList<ChatLog.Line>() : ArcticClient.chatLog().search(f, LINES);
	}

	@Override
	public void render(Gfx g, Style s, boolean preview) {
		refresh();
		List<String> rows = new ArrayList<String>();
		if (filter() == null) {
			if (!preview) {
				return;
			}
			rows.add("Pick a filter in Chat history");
		} else if (shown.isEmpty()) {
			if (!preview) {
				return;
			}
			rows.add("Lines that match \"" + filter().name + "\" show here");
		} else {
			// Newest at the bottom, like a chat.
			for (int i = shown.size() - 1; i >= 0; i--) {
				rows.add(shown.get(i).text);
			}
		}
		panel(g, s, 0, 0, WIDTH, PAD + rows.size() * LINE);
		int y = PAD / 2 + 1;
		for (String row : rows) {
			g.text(Draw.fit(g, row, WIDTH - PAD * 2), PAD, y, text(), shadow());
			y += LINE;
		}
	}
}
