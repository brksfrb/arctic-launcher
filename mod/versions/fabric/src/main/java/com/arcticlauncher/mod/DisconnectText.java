package com.arcticlauncher.mod;

import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

/**
 * What the game's "Connection lost" screen says, for Arctic's in its place. Its fields moved
 * between versions (a reason, later a record holding it), so they're found by type: the text
 * that isn't the title, or a value with a {@code reason()} giving one.
 */
public final class DisconnectText {
	private DisconnectText() {}

	public static String reason(Screen screen) {
		Component title = screen.getTitle();
		for (Class<?> c = screen.getClass(); c != null && c != Screen.class; c = c.getSuperclass()) {
			for (java.lang.reflect.Field f : c.getDeclaredFields()) {
				if (java.lang.reflect.Modifier.isStatic(f.getModifiers())) {
					continue;
				}
				try {
					f.setAccessible(true);
					Object value = f.get(screen);
					if (value instanceof Component && value != title) {
						return ((Component) value).getString();
					}
					if (value != null && !(value instanceof Screen)) {
						for (java.lang.reflect.Method m : value.getClass().getMethods()) {
							if (m.getParameterCount() == 0 && Component.class.isAssignableFrom(m.getReturnType())
									&& m.getDeclaringClass() == value.getClass()) {
								Object reason = m.invoke(value);
								if (reason != null) {
									return ((Component) reason).getString();
								}
							}
						}
					}
				} catch (ReflectiveOperationException | RuntimeException ignored) {
					// Not this one.
				}
			}
		}
		return "";
	}
}
