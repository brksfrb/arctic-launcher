package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class ChatStampWidthTest {
	/** A proportional font like the game's: digit 1 is 1px narrower, bold adds 1px per glyph, formatting codes are free. */
	private static final ChatStampWidth.Measure FONT = s -> {
		float w = 0;
		boolean bold = false;
		for (int i = 0; i < s.length(); i++) {
			char ch = s.charAt(i);
			if (ch == '§') {
				char code = s.charAt(++i);
				bold = code == 'l' || bold && code != 'r';
				continue;
			}
			w += (ch == '1' ? 4.375f : ch == ':' || ch == '[' || ch == ']' ? 3 : ch == ' ' ? 4 : 6) + (bold ? 1 : 0);
		}
		return w;
	};

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
						: new String[] {"11:11:11", "08:58:08", "22:31:15", "22:31:57", "23:59:59"};
				int slot = ChatStampWidth.slot(c, FONT, FONT);
				for (String time : times) {
					float stamp = FONT.width((bold ? "§l" : "") + "[" + time + "] ");
					assertTrue(stamp <= slot);
					float pen = stamp;
					for (char ch : ChatStampWidth.spacer(slot - stamp).toCharArray()) {
						pen += ChatStampWidth.advance(ch);
					}
					assertEquals((float) slot, pen, time);
				}
			}
		}
	}

	@Test
	void spacerRepresentsSmallIntegerAndFractionalGaps() {
		for (float gap : new float[] {0, 1, 2, 7, 16.0625f, 63.999996f}) {
			float total = 0;
			for (char ch : ChatStampWidth.spacer(gap).toCharArray()) {
				total += ChatStampWidth.advance(ch);
			}
			assertEquals(gap, total);
		}
	}

	@Test
	void refitsWhenTheFontChanges() {
		ClientConfig c = new ClientConfig();
		int wide = ChatStampWidth.slot(c, "a", FONT);
		ChatStampWidth.Measure thin = s -> FONT.width(s) * 0.75f;
		assertNotEquals(wide, ChatStampWidth.slot(c, "b", thin));
	}
}
