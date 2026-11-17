package com.arcticlauncher.client.feature;

/**
 * Chat archive lines written by the modern versions carry two extra markers: {@code §@namespace:font;} and
 * {@code §#rrggbb}. A renderer that knows only the sixteen legacy colours (1.8.9) turns them into plain
 * legacy codes here; otherwise its formatter consumes only the marker characters and shows the rest as text.
 */
public final class ChatArchiveText {
	private ChatArchiveText() {}

	private static final int[] PALETTE = {
		0x000000, 0x0000aa, 0x00aa00, 0x00aaaa, 0xaa0000, 0xaa00aa, 0xffaa00, 0xaaaaaa,
		0x555555, 0x5555ff, 0x55ff55, 0x55ffff, 0xff5555, 0xff55ff, 0xffff55, 0xffffff,
	};

	/** The text with fonts dropped and hex colours replaced by the nearest legacy colour code. */
	public static String toLegacy(String text) {
		if (text == null || text.indexOf("§@") < 0 && text.indexOf("§#") < 0) return text;
		StringBuilder out = new StringBuilder(text.length());
		int n = text.length();
		for (int i = 0; i < n; i++) {
			char ch = text.charAt(i);
			if (ch != '§' || i + 1 >= n) { out.append(ch); continue; }
			char code = text.charAt(i + 1);
			if (code == '@') {
				int end = text.indexOf(';', i + 2);
				// A font id never holds a section sign; one in the way means no terminator was written.
				int stop = text.indexOf('§', i + 2);
				i = end >= 0 && (stop < 0 || end < stop) ? end : i + 1;
			} else if (code == '#') {
				int rgb = hex(text, i + 2);
				if (rgb >= 0) { out.append('§').append(Character.forDigit(nearest(rgb), 16)); i += 7; }
				else i += 1;
			} else {
				out.append(ch).append(code);
				i++;
			}
		}
		return out.toString();
	}

	private static int hex(String text, int from) {
		if (from + 6 > text.length()) return -1;
		int v = 0;
		for (int i = from; i < from + 6; i++) {
			int d = Character.digit(text.charAt(i), 16);
			if (d < 0) return -1;
			v = v << 4 | d;
		}
		return v;
	}

	static int nearest(int rgb) {
		int best = 15;
		long bestDistance = Long.MAX_VALUE;
		for (int i = 0; i < PALETTE.length; i++) {
			long dr = (rgb >> 16 & 255) - (PALETTE[i] >> 16 & 255);
			long dg = (rgb >> 8 & 255) - (PALETTE[i] >> 8 & 255);
			long db = (rgb & 255) - (PALETTE[i] & 255);
			long d = dr * dr + dg * dg + db * db;
			if (d < bestDistance) { bestDistance = d; best = i; }
		}
		return best;
	}
}
