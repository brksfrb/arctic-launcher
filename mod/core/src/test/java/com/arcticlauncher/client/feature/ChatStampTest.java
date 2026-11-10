package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.LocalClock;
import com.arcticlauncher.client.config.ClientConfig;
import java.time.Instant;
import java.util.Date;
import java.util.SimpleTimeZone;
import java.util.TimeZone;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class ChatStampTest {
	@Test
	void osOffsetWinsOverStaleIstanbulWinterRules() {
		TimeZone original = TimeZone.getDefault();
		String property = System.getProperty(LocalClock.OFFSET_PROPERTY);
		try {
			TimeZone.setDefault(new SimpleTimeZone(2 * 3600000, "Stale Europe/Istanbul winter"));
			System.setProperty(LocalClock.OFFSET_PROPERTY, "10800");
			Date now = Date.from(Instant.parse("2026-11-10T03:25:12Z"));
			ClientConfig c = new ClientConfig();
			c.chatStamp12h = false;
			c.chatStampSeconds = false;
			c.chatStampAmPm = false;
			c.chatStampBrackets = true;
			assertEquals("[06:25] ", ChatStamp.text(c, now));
			c.chatStamp12h = true;
			c.chatStampSeconds = true;
			c.chatStampAmPm = true;
			c.chatStampBold = true;
			c.chatStampItalic = true;
			assertEquals("[6:25:12 AM] ", ChatStamp.text(c, now));
			c.chatStampBrackets = false;
			assertEquals("6:25:12 AM ", ChatStamp.text(c, now));
			// The public entry point formats the actual current instant with the OS offset too.
			c.chatStamp12h = false;
			c.chatStampSeconds = false;
			c.chatStampAmPm = false;
			String before = LocalClock.formatter("HH:mm").format(new Date()) + " ";
			String stamp = ChatStamp.text(c);
			String after = LocalClock.formatter("HH:mm").format(new Date()) + " ";
			assertTrue(stamp.equals(before) || stamp.equals(after));
			// Date rollover and fractional/negative offsets use the same helper as the HUD.
			Date midnight = Date.from(Instant.parse("2026-11-10T21:25:12Z"));
			assertEquals("Wed 11 Nov 00:25", LocalClock.formatter("EEE d MMM HH:mm", java.util.Locale.ENGLISH).format(midnight));
			System.setProperty(LocalClock.OFFSET_PROPERTY, "20700");
			assertEquals("09:10", LocalClock.formatter("HH:mm").format(now));
			System.setProperty(LocalClock.OFFSET_PROPERTY, "-12600");
			assertEquals("23:55", LocalClock.formatter("HH:mm").format(now));
			for (String invalid : new String[] {"bad", "64801", "-64801", "2147483647"}) {
				System.setProperty(LocalClock.OFFSET_PROPERTY, invalid);
				assertEquals("05:25", LocalClock.formatter("HH:mm").format(now));
			}
			System.clearProperty(LocalClock.OFFSET_PROPERTY);
			assertEquals("05:25", LocalClock.formatter("HH:mm").format(now));
		} finally {
			TimeZone.setDefault(original);
			if (property == null) System.clearProperty(LocalClock.OFFSET_PROPERTY);
			else System.setProperty(LocalClock.OFFSET_PROPERTY, property);
		}
	}
}
