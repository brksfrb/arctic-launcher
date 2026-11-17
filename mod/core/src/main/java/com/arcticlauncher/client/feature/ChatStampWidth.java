package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;

/**
 * Pads a chat timestamp to the width of the widest time the current format and font can show,
 * so every message starts in the same column although the font is proportional ("11:11" is
 * narrower than "08:58"). The padding is plain spaces, normal and bold mixed (a bold space is
 * 1px wider) so the slot fits to the pixel; the caller keeps it outside the click/copy target.
 */
public final class ChatStampWidth {
	private ChatStampWidth() {}

	/** Pixel width of a string with section-sign formatting codes, in the game's own font. */
	public interface Measure {
		float width(String text);
	}

	/** Proof scenes only: put the padding before the time (right-aligned) instead of after it. */
	public static volatile boolean proofRight;
	/** Proof scenes only: no padding at all, the behaviour before the fixed slot. */
	public static volatile boolean proofOff;

	// The slot: recomputed when the font object, the format, or the glyph widths change
	// (a resource pack or custom font reload), otherwise a field compare per message.
	private static Object slotFont;
	private static int slotFlags = -1;
	private static float slotProbe = -1;
	private static float slotWidth;
	private static float[] pieceWidth = new float[0];
	private static String[] pieceText = new String[0];

	/** Spaces that fill a gap: normal and bold (1px wider), and in a proportional font the thin and hair spaces. */
	private static final String[] SPACES = {" ", "§l ", "\u2009", "\u200a"};

	private static final String DIGITS = "0123456789";

	/** The padding (with formatting codes) that completes {@code stamp} to the slot; "" when none is needed. */
	public static String padding(ClientConfig c, Object font, Measure m, String stamp) {
		if (proofOff) {
			return "";
		}
		String bold = c.chatStampBold ? "§l" : "";
		int flags = (c.chatStamp12h ? 1 : 0) | (c.chatStampSeconds ? 2 : 0) | (c.chatStampAmPm ? 4 : 0)
				| (c.chatStampBrackets ? 8 : 0) | (c.chatStampBold ? 16 : 0) | (c.fancy ? 32 : 0);
		float probe = m.width(bold + DIGITS + "[]: AMP");
		if (font != slotFont || flags != slotFlags || probe != slotProbe) {
			slotFont = font;
			slotFlags = flags;
			slotProbe = probe;
			slotWidth = widest(c, m, bold);
			int n = 0;
			float[] widths = new float[SPACES.length];
			String[] texts = new String[SPACES.length];
			for (int i = 0; i < SPACES.length; i++) {
				// Thin spaces only exist in the smooth font; elsewhere they would draw as a missing-glyph box.
				if (i >= 2 && !c.fancy) {
					continue;
				}
				float w = m.width("§r" + SPACES[i]);
				if (w > 0 && w <= 8) {
					widths[n] = w;
					texts[n++] = SPACES[i];
				}
			}
			pieceWidth = java.util.Arrays.copyOf(widths, n);
			pieceText = java.util.Arrays.copyOf(texts, n);
		}
		float missing = slotWidth - m.width(bold + stamp);
		if (missing <= 0 || pieceWidth.length == 0) {
			return "";
		}
		// Take the mix of pieces that lands closest to the slot, over or under, then the fewest characters.
		int[] best = new int[pieceWidth.length];
		fit(0, 0f, 0, missing, new int[pieceWidth.length], best, new float[] {missing}, new int[1]);
		StringBuilder out = new StringBuilder("§r");
		for (int i = 0; i < best.length; i++) {
			if (best[i] > 0) {
				// "§r" before each piece keeps its own weight whatever the stamp's was.
				for (int k = 0; k < best[i]; k++) {
					out.append("§r").append(pieceText[i]);
				}
			}
		}
		return out.append("§r").toString();
	}

	/** Brute force over how many of each piece: few pieces, small counts, once per message. */
	private static void fit(int piece, float sum, int used, float missing, int[] count, int[] best, float[] bestOff, int[] bestChars) {
		if (piece == pieceWidth.length) {
			float off = Math.abs(missing - sum);
			if (off < bestOff[0] - 0.001f || off < bestOff[0] + 0.001f && used < bestChars[0]) {
				bestOff[0] = off;
				bestChars[0] = used;
				System.arraycopy(count, 0, best, 0, count.length);
			}
			return;
		}
		int max = (int) (missing / pieceWidth[piece]) + 1;
		if (piece >= 2) {
			max = Math.min(max, 3);
		}
		for (int k = 0; k <= max; k++) {
			count[piece] = k;
			fit(piece + 1, sum + k * pieceWidth[piece], used + k, missing, count, best, bestOff, bestChars);
		}
		count[piece] = 0;
	}

	public static boolean right() {
		return proofRight;
	}

	/**
	 * Width of the widest time the format can show: the widest hour, the widest digit per minute and
	 * second position, the wider of AM/PM, measured as one assembled string (measuring the parts
	 * separately would round each up).
	 */
	static float widest(ClientConfig c, Measure m, String bold) {
		StringBuilder hour = new StringBuilder();
		String best = "";
		float bestWidth = -1;
		for (int h = c.chatStamp12h ? 1 : 0; h <= (c.chatStamp12h ? 12 : 23); h++) {
			String candidate = (c.chatStamp12h || h > 9 ? "" : "0") + h;
			float w = m.width(bold + candidate);
			if (w > bestWidth) {
				bestWidth = w;
				best = candidate;
			}
		}
		hour.append(best);
		char tens = widestDigit(m, bold, 5);
		char units = widestDigit(m, bold, 9);
		hour.append(':').append(tens).append(units);
		if (c.chatStampSeconds) {
			hour.append(':').append(tens).append(units);
		}
		if (c.chatStampAmPm) {
			hour.append(m.width(bold + " AM") >= m.width(bold + " PM") ? " AM" : " PM");
		}
		String time = hour.toString();
		return m.width(bold + (c.chatStampBrackets ? "[" + time + "]" : time) + " ");
	}

	/** The widest digit 0 through {@code max}. */
	private static char widestDigit(Measure m, String bold, int max) {
		char best = '0';
		float bestWidth = -1;
		for (int d = 0; d <= max; d++) {
			float w = m.width(bold + d);
			if (w > bestWidth) {
				bestWidth = w;
				best = (char) ('0' + d);
			}
		}
		return best;
	}
}
