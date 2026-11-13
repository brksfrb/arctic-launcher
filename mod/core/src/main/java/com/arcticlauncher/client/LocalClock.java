package com.arcticlauncher.client;

import java.text.SimpleDateFormat;
import java.util.Locale;
import java.util.SimpleTimeZone;
import java.util.TimeZone;

/** Local wall time using the OS offset supplied by the launcher, even on old Java tzdbs.
 * The offset is fixed for this game session; restart after a DST or OS time-zone change.
 */
public final class LocalClock {
	public static final String OFFSET_PROPERTY = "arctic.localUtcOffsetSeconds";

	private LocalClock() {}

	public static SimpleDateFormat formatter(String pattern) {
		return formatter(pattern, Locale.getDefault());
	}

	public static SimpleDateFormat formatter(String pattern, Locale locale) {
		SimpleDateFormat format = new SimpleDateFormat(pattern, locale);
		format.setTimeZone(zone());
		return format;
	}

	private static TimeZone zone() {
		String supplied = System.getProperty(OFFSET_PROPERTY);
		if (supplied != null) {
			try {
				int seconds = Integer.parseInt(supplied);
				if (seconds >= -18 * 3600 && seconds <= 18 * 3600) {
					// SimpleTimeZone has a fixed raw offset and no tzdb/DST rules.
					return new SimpleTimeZone(seconds * 1000, "Arctic OS local offset"); // i18n-ignore
				}
			} catch (NumberFormatException ignored) {}
		}
		// Standalone development or a third-party launcher without the property.
		return TimeZone.getDefault();
	}
}
