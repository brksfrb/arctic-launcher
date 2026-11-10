package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.LocalClock;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.util.Date;
import java.util.Locale;
import java.util.SimpleTimeZone;
import java.util.TimeZone;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class LocalClockWidgetTest {
	@Test
	void clockAndDateWidgetsUseOsOffsetInsteadOfJvmDefault() throws Exception {
		Field platform = ArcticClient.class.getDeclaredField("platform");
		platform.setAccessible(true);
		Object originalPlatform = platform.get(null);
		TimeZone original = TimeZone.getDefault();
		String property = System.getProperty(LocalClock.OFFSET_PROPERTY);
		try {
			platform.set(null, Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class},
					(proxy, method, args) -> method.getReturnType() == boolean.class ? false : null));
			TimeZone.setDefault(new SimpleTimeZone(2 * 3600000, "Stale Europe/Istanbul winter"));
			System.setProperty(LocalClock.OFFSET_PROPERTY, "10800");
			int checked = 0;
			for (HudWidget widget : Widgets.all(new Cps())) {
				String pattern;
				if (widget.id.equals("clock")) pattern = "HH:mm";
				else if (widget.id.equals("date")) pattern = "EEE d MMM";
				else continue;
				String before = LocalClock.formatter(pattern, Locale.ENGLISH).format(new Date());
				String shown = ((TextWidget) widget).value(false);
				String after = LocalClock.formatter(pattern, Locale.ENGLISH).format(new Date());
				assertTrue(shown.equals(before) || shown.equals(after), widget.id + ": " + shown);
				checked++;
			}
			assertEquals(2, checked);
		} finally {
			platform.set(null, originalPlatform);
			TimeZone.setDefault(original);
			if (property == null) System.clearProperty(LocalClock.OFFSET_PROPERTY);
			else System.setProperty(LocalClock.OFFSET_PROPERTY, property);
		}
	}
}
