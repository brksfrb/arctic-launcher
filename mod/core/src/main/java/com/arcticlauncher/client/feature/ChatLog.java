package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ChatFilter;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * Every chat line since the game started (up to {@link #MAX}), kept apart from the game's own
 * chat so it can be searched and filtered, and survives joining another server. Lines are plain
 * text (no formatting codes).
 */
public final class ChatLog {
	/** Lines kept; the oldest go first. */
	public static final int MAX = 50_000;

	/** One line: when, on which server, and what it said. */
	public static final class Line {
		public final long time;
		public final String server;
		public final String text;
		private final String lower;

		Line(long time, String server, String text) {
			this.time = time;
			this.server = server;
			this.text = text;
			this.lower = text.toLowerCase(Locale.ROOT);
		}
	}

	private final Line[] ring = new Line[MAX];
	private int start;
	private int size;
	private long added;

	/** Add a line (formatting codes are taken out; empty lines are skipped). */
	public synchronized void add(String text, String server) {
		if (text == null) {
			return;
		}
		String plain = strip(text).trim();
		if (plain.isEmpty()) {
			return;
		}
		Line line = new Line(System.currentTimeMillis(), server == null ? "" : server, plain);
		if (size < MAX) {
			ring[(start + size) % MAX] = line;
			size++;
		} else {
			ring[start] = line;
			start = (start + 1) % MAX;
		}
		added++;
	}

	/** How many lines were ever added (changes whenever a line arrives). */
	public synchronized long added() {
		return added;
	}

	public synchronized int size() {
		return size;
	}

	public synchronized void clear() {
		java.util.Arrays.fill(ring, null);
		start = 0;
		size = 0;
		added++;
	}

	/**
	 * The newest lines that match {@code query} (a filter's words, or text typed in a search
	 * box), newest first, at most {@code limit}.
	 */
	public synchronized List<Line> search(String query, int limit) {
		Matcher m = new Matcher(query);
		List<Line> out = new ArrayList<Line>();
		for (int i = size - 1; i >= 0 && out.size() < limit; i--) {
			Line line = ring[(start + i) % MAX];
			if (m.matches(line.lower)) {
				out.add(line);
			}
		}
		return out;
	}

	/** Lines that match both a saved filter's words and what was typed in a search box. */
	public synchronized List<Line> search(String filterWords, String typed, int limit) {
		Matcher filter = new Matcher(filterWords);
		Matcher search = new Matcher(typed);
		List<Line> out = new ArrayList<Line>();
		for (int i = size - 1; i >= 0 && out.size() < limit; i--) {
			Line line = ring[(start + i) % MAX];
			if (filter.matches(line.lower) && search.matches(line.lower)) {
				out.add(line);
			}
		}
		return out;
	}

	/** The newest matching lines of a saved filter, newest first. */
	public List<Line> search(ChatFilter filter, int limit) {
		return search(filter == null ? "" : filter.words, limit);
	}

	/** "diamond, trade, -spam": any of the words, none of the "-" ones; empty matches everything. */
	static final class Matcher {
		private final List<String> any = new ArrayList<String>();
		private final List<String> none = new ArrayList<String>();

		Matcher(String query) {
			if (query == null) {
				return;
			}
			for (String part : query.split(",")) {
				String word = part.trim().toLowerCase(Locale.ROOT);
				if (word.length() > 1 && word.charAt(0) == '-') {
					none.add(word.substring(1).trim());
				} else if (!word.isEmpty()) {
					any.add(word);
				}
			}
		}

		boolean matches(String lower) {
			for (String word : none) {
				if (!word.isEmpty() && lower.contains(word)) {
					return false;
				}
			}
			if (any.isEmpty()) {
				return true;
			}
			for (String word : any) {
				if (lower.contains(word)) {
					return true;
				}
			}
			return false;
		}
	}

	/** Without the game's formatting codes (a section sign and the character after it). */
	static String strip(String text) {
		if (text.indexOf('§') < 0) {
			return text;
		}
		StringBuilder out = new StringBuilder(text.length());
		for (int i = 0; i < text.length(); i++) {
			char c = text.charAt(i);
			if (c == '§') {
				i++;
			} else {
				out.append(c);
			}
		}
		return out.toString();
	}
}
