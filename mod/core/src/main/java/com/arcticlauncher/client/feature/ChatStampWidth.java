package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;

/** Width of the chat timestamp column and the invisible advances used by chat layout. */
public final class ChatStampWidth {
	private ChatStampWidth() {}

	public interface Measure {
		float width(String text);
	}

	// One private-use glyph per power of two, shared by the font and layout adapters.
	public static final int FIRST = 0xE800;
	public static final int MIN_POWER = -20;
	public static final int MAX_POWER = 7;

	public static float advance(int codepoint) {
		int power = codepoint - FIRST + MIN_POWER;
		return power >= MIN_POWER && power <= MAX_POWER ? Math.scalb(1f, power) : 0f;
	}

	public static String spacer(float width) {
		StringBuilder text = new StringBuilder();
		for (int power = MAX_POWER; power >= MIN_POWER && width > 0; power--) {
			float step = Math.scalb(1f, power);
			while (width >= step) {
				text.append((char) (FIRST + power - MIN_POWER));
				width -= step;
			}
		}
		return text.toString();
	}

	private static final String DIGITS = "0123456789";
	private static Object slotFont;
	private static int slotFlags = -1;
	private static float slotProbe;
	private static int slotWidth;

	public static int slot(ClientConfig c, Object font, Measure measure) {
		String bold = c.chatStampBold ? "§l" : "";
		int flags = (c.chatStamp12h ? 1 : 0) | (c.chatStampSeconds ? 2 : 0) | (c.chatStampAmPm ? 4 : 0)
				| (c.chatStampBrackets ? 8 : 0) | (c.chatStampBold ? 16 : 0) | (c.fancy ? 32 : 0);
		float probe = measure.width(bold + DIGITS + "[]: AMP");
		if (font != slotFont || flags != slotFlags || probe != slotProbe) {
			slotFont = font;
			slotFlags = flags;
			slotProbe = probe;
			slotWidth = (int) Math.ceil(widest(c, measure, bold));
		}
		return slotWidth;
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
