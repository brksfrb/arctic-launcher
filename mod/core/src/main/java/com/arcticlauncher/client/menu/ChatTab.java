package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ChatFilter;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.ChatLog;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Label;
import com.arcticlauncher.client.ui.TextField;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.List;

import com.arcticlauncher.client.Lang;
/**
 * Chat history: every line since the game started (across servers), searched as you type and
 * narrowed by saved filters. A filter can also be shown on screen as a second chat (the
 * "Filtered chat" HUD widget). The game's own chat keeps as many lines as the history setting.
 */
final class ChatTab implements MenuTab {
	private static final int FIELD_H = 18;
	private static final int LINE_H = 11;
	private static final int GAP = 4;
	private static final int REMOVE_W = 20;
	private static final int SHOWN = 150;
	private static final int MAX_FILTERS = 12;
	private static final int MAX_TEXT = 120;
	private static final int[] HISTORY = {100, 500, 1000, 5000, 10000, 25000, 100000};

	private final TextField search = new TextField(Lang.t("social.search_the_chat"), MAX_TEXT, TextField.ANY);
	/** 0 = everything, n = the n-th saved filter. */
	private int view;
	private final SimpleDateFormat time = new SimpleDateFormat("HH:mm:ss");

	@Override
	public String title() {
		return Lang.t("social.chat_history");
	}

	@Override
	public String hint() {
		return Lang.t("social.search_every_line_or_keep_a_filter_as_a");
	}

	@Override
	public String state() {
		ClientConfig c = ArcticClient.config();
		return ArcticClient.chatLog().added() + "|" + view + "|" + search.text() + "|" + c.chatFilters.size()
				+ "|" + c.chatHistory + "|" + c.chatFeedFilter;
	}

	@Override
	public boolean rebuildWhileTyping() {
		return true;
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		final ClientConfig c = ArcticClient.config();
		final ChatLog log = ArcticClient.chatLog();
		if (view > c.chatFilters.size()) {
			view = 0;
		}
		Form f = new Form(host, x, top, w, bottom);
		f.section(Lang.t("social.find"));
		int row = f.row(FIELD_H + GAP);
		f.put(search, f.x, row, f.w, FIELD_H);
		// "All" and each saved filter, to narrow the search to.
		int chips = c.chatFilters.size() + 1;
		int chipsRow = f.row(FIELD_H + GAP);
		int cw = Math.min(90, (f.w - GAP * (chips - 1)) / chips);
		for (int i = 0; i < chips; i++) {
			final int index = i;
			String name = i == 0 ? Lang.t("social.all") : c.chatFilters.get(i - 1).name;
			f.put(new Button(name.isEmpty() ? "(no name)" : name, () -> {
				view = index;
				host.rebuild();
			}).selected(view == i), f.x + i * (cw + GAP), chipsRow, cw, FIELD_H);
		}
		final String words = view == 0 ? "" : c.chatFilters.get(view - 1).words;
		final List<ChatLog.Line> lines = log.search(words, search.text(), SHOWN);
		f.section(Lang.t("social.lines") + log.size() + Lang.t("social.kept_newest_first"));
		if (lines.isEmpty()) {
			f.note(log.size() == 0 ? Lang.t("social.nothing_has_been_said_yet") : Lang.t("social.nothing_matches"));
		}
		for (ChatLog.Line line : lines) {
			String shown = "§8" + time.format(new Date(line.time)) + " §r" + line.text;
			f.put(new Label(shown, Label.Kind.TEXT), f.x + 4, f.row(LINE_H), f.w - 4, LINE_H);
		}
		f.gap(4);
		int actions = f.row(FIELD_H + GAP);
		int bw = (f.w - GAP * 2) / 3;
		f.put(new Button("Copy these lines", () -> {
			StringBuilder out = new StringBuilder();
			for (int i = lines.size() - 1; i >= 0; i--) {
				out.append(lines.get(i).text).append('\n');
			}
			ArcticClient.platform().setClipboard(out.toString());
			com.arcticlauncher.client.notice.Notices.post(Lang.t("social.copied"), lines.size() + " lines");
		}), f.x, actions, bw, FIELD_H);
		f.put(new Button("Save as a filter", () -> {
			String typed = search.text().trim();
			if (typed.isEmpty() || c.chatFilters.size() >= MAX_FILTERS) {
				return;
			}
			c.chatFilters.add(new ChatFilter(typed.length() > 16 ? typed.substring(0, 16) : typed, typed));
			ArcticClient.saveConfig();
			search.text("");
			host.rebuild();
		}), f.x + bw + GAP, actions, bw, FIELD_H);
		f.put(new Button("Clear the log", () -> {
			log.clear();
			host.rebuild();
		}).confirm(Lang.t("social.sure")), f.x + (bw + GAP) * 2, actions, f.w - (bw + GAP) * 2, FIELD_H);

		f.section(Lang.t("social.filters"));
		f.note(Lang.t("social.words_separated_by_commas_a_line_with_any_of"));
		f.note(Lang.t("social.show_one_on_screen_as_a_second_chat_with_the"));
		for (final ChatFilter filter : c.chatFilters) {
			int r = f.row(FIELD_H + GAP);
			final TextField name = new TextField(Lang.t("social.name"), 24, TextField.ANY).text(filter.name);
			name.onChange(() -> {
				filter.name = name.text();
				ArcticClient.saveConfig();
			});
			int nameW = 90;
			f.put(name, f.x, r, nameW, FIELD_H);
			final TextField wordsField = new TextField("diamond, trade, -spam", MAX_TEXT, TextField.ANY).text(filter.words);
			wordsField.onChange(() -> {
				filter.words = wordsField.text();
				ArcticClient.saveConfig();
			});
			int showW = 50;
			f.put(wordsField, f.x + nameW + GAP, r, f.w - nameW - showW - REMOVE_W - GAP * 3, FIELD_H);
			boolean on = filter.name.equals(c.chatFeedFilter);
			f.put(new Button(on ? "On HUD" : "Show", () -> {
				c.chatFeedFilter = filter.name.equals(c.chatFeedFilter) ? "" : filter.name;
				ArcticClient.saveConfig();
				host.rebuild();
			}).selected(on), f.x + f.w - REMOVE_W - GAP - showW, r, showW, FIELD_H);
			f.put(new Button("×", () -> {
				c.chatFilters.remove(filter);
				if (filter.name.equals(c.chatFeedFilter)) {
					c.chatFeedFilter = "";
				}
				ArcticClient.saveConfig();
				host.rebuild();
			}), f.x + f.w - REMOVE_W, r, REMOVE_W, FIELD_H);
		}
		if (c.chatFilters.size() < MAX_FILTERS) {
			int r = f.row(FIELD_H + GAP);
			f.put(new Button("Add a filter", () -> {
				c.chatFilters.add(new ChatFilter(Lang.t("social.filter") + (c.chatFilters.size() + 1), ""));
				ArcticClient.saveConfig();
				host.rebuild();
			}), f.x, r, 120, FIELD_H);
		}

		f.section(Lang.t("social.history"));
		String[] labels = new String[HISTORY.length];
		for (int i = 0; i < labels.length; i++) {
			labels[i] = HISTORY[i] + (HISTORY[i] == 100 ? " (game)" : "");
		}
		f.cycle(Lang.t("social.chat_keeps"), labels, () -> indexOf(c.chatHistory), i -> c.chatHistory = HISTORY[i]);
		f.note(Lang.t("social.how_many_lines_the_games_chat_window_can"));
		f.note(Lang.t("social.this_list_keeps_the_last") + ChatLog.MAX + Lang.t("social.lines_whatever_you_choose"));
		f.done();
	}

	private static int indexOf(int value) {
		int best = 0;
		for (int i = 0; i < HISTORY.length; i++) {
			if (HISTORY[i] <= value) {
				best = i;
			}
		}
		return best;
	}
}
