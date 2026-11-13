package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class ChatStampWidthTest {
	/** A proportional font like the game's: digit 1 is 1px narrower, bold adds 1px per glyph, formatting codes are free. */
	private static final ChatStampWidth.Measure FONT = s -> {
		int w = 0;
		boolean bold = false;
		for (int i = 0; i < s.length(); i++) {
			char ch = s.charAt(i);
			if (ch == '§') {
				char code = s.charAt(++i);
				bold = code == 'l' || bold && code != 'r';
				continue;
			}
			w += (ch == '\u2009' ? 2 : ch == '\u200a' ? 1 : ch == '1' ? 5 : ch == ':' || ch == '[' || ch == ']' ? 3 : ch == ' ' ? 4 : 6) + (bold ? 1 : 0);
		}
		return w;
	};

	private static float width(String s) {
		return FONT.width(s);
	}

	@Test
	void everyTimeFillsTheSameSlot() {
		for (boolean bold : new boolean[] {false, true}) {
			for (boolean h12 : new boolean[] {false, true}) {
				ClientConfig c = new ClientConfig();
				c.chatStamp12h = h12;
				c.chatStampBold = bold;
				c.chatStampBrackets = true;
				c.chatStampSeconds = true;
				c.chatStampAmPm = h12;
				String[] times = h12 ? new String[] {"1:11:11 AM", "8:58:08 PM", "12:59:59 PM", "10:00:00 AM"}
						: new String[] {"11:11:11", "08:58:08", "20:00:00", "19:19:19", "23:59:59"};
				float expected = -1;
				for (String t : times) {
					String stamp = "[" + t + "] ";
					String pad = ChatStampWidth.padding(c, "font", FONT, stamp);
					float total = width((bold ? "§l" : "") + stamp) + width(pad);
					if (expected < 0) {
						expected = width((bold ? "§l" : "") + (h12 ? "[12:59:59 PM] " : "[00:00:00] "));
					}
					// Spaces come in 4px and 5px (bold): any gap fits to within 2px of the slot.
					assertTrue(Math.abs(total - expected) <= 2f, t + " bold=" + bold + " " + total + " vs " + expected);
				}
			}
		}
	}

	@Test
	void thinSpacesTightenTheFitInTheSmoothFont() {
		ClientConfig c = new ClientConfig();
		c.chatStampBrackets = true;
		c.fancy = true;
		float slot = width("[00:00] ");
		for (String t : new String[] {"11:11", "19:19", "12:12", "01:01", "08:58"}) {
			String stamp = "[" + t + "] ";
			float total = width(stamp) + width(ChatStampWidth.padding(c, "fancy", FONT, stamp));
			assertTrue(Math.abs(total - slot) <= 0.5f, t + " " + total + " vs " + slot);
		}
	}

	@Test
	void widestTimeGetsNoPadding() {
		ClientConfig c = new ClientConfig();
		c.chatStampBrackets = true;
		assertEquals("", ChatStampWidth.padding(c, "font", FONT, "[00:00] "));
		assertFalse(ChatStampWidth.padding(c, "font", FONT, "[11:11] ").isEmpty());
	}

	@Test
	void refitsWhenTheFontChanges() {
		ClientConfig c = new ClientConfig();
		String stamp = "[11:11] ";
		String narrow = ChatStampWidth.padding(c, "a", FONT, stamp);
		ChatStampWidth.Measure thin = s -> FONT.width(s) - s.chars().filter(ch -> ch == '1').count() * 3;
		assertNotEquals(narrow, ChatStampWidth.padding(c, "b", thin, stamp));
	}
}
