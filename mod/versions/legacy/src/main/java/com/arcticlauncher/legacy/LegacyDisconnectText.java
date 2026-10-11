package com.arcticlauncher.legacy;

import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;

/**
 * What the game's "Connection lost" screen says (title, reason), for Arctic's in its place: a
 * translation key for the title and a text for the reason, found by type.
 */
public final class LegacyDisconnectText {
	private LegacyDisconnectText() {}

	public static String[] read(Screen screen) {
		String title = "";
		String reason = "";
		for (java.lang.reflect.Field f : screen.getClass().getDeclaredFields()) {
			if (java.lang.reflect.Modifier.isStatic(f.getModifiers())) {
				continue;
			}
			try {
				f.setAccessible(true);
				Object value = f.get(screen);
				if (value instanceof String && title.isEmpty()) {
					title = net.minecraft.client.resource.language.I18n.translate((String) value);
				} else if (value instanceof Text && reason.isEmpty()) {
					reason = ((Text) value).asUnformattedString();
				}
			} catch (ReflectiveOperationException | RuntimeException ignored) {
				// Not this one.
			}
		}
		return new String[] {title, reason};
	}
}
