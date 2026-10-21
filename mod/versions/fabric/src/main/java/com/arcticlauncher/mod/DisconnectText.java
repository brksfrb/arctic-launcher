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

	/** The text with its colors as § codes (the nearest of the game's 16), so Arctic's page shows them. */
	static String formatted(Component component) {
		//#if MC >= 1.16
		StringBuilder out = new StringBuilder();
		component.visit((style, text) -> {
			net.minecraft.network.chat.TextColor color = style.getColor();
			out.append("§r");
			if (color != null) {
				out.append(nearest(color.getValue()));
			}
			if (style.isBold()) {
				out.append("§l");
			}
			out.append(text);
			return java.util.Optional.empty();
		}, net.minecraft.network.chat.Style.EMPTY);
		return out.toString();
		//#else
		return component.getString();
		//#endif
	}

	private static final int[] PALETTE = {0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xFFAA00, 0xAAAAAA,
			0x555555, 0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF};

	/** "§c" for the color closest to {@code rgb}. */
	private static String nearest(int rgb) {
		int best = 0;
		long bestDistance = Long.MAX_VALUE;
		for (int i = 0; i < PALETTE.length; i++) {
			long dr = ((rgb >> 16) & 255) - ((PALETTE[i] >> 16) & 255);
			long dg = ((rgb >> 8) & 255) - ((PALETTE[i] >> 8) & 255);
			long db = (rgb & 255) - (PALETTE[i] & 255);
			long distance = dr * dr + dg * dg + db * db;
			if (distance < bestDistance) {
				bestDistance = distance;
				best = i;
			}
		}
		return "§" + Integer.toHexString(best);
	}

	public static String reason(Screen screen) {
		Component title = Compat.titleOf(screen);
		for (Class<?> c = screen.getClass(); c != null && c != Screen.class; c = c.getSuperclass()) {
			for (java.lang.reflect.Field f : c.getDeclaredFields()) {
				if (java.lang.reflect.Modifier.isStatic(f.getModifiers())) {
					continue;
				}
				try {
					f.setAccessible(true);
					Object value = f.get(screen);
					if (value instanceof Component && value != title) {
						return formatted((Component) value);
					}
					if (value != null && !(value instanceof Screen)) {
						for (java.lang.reflect.Method m : value.getClass().getMethods()) {
							if (m.getParameterCount() == 0 && Component.class.isAssignableFrom(m.getReturnType())
									&& m.getDeclaringClass() == value.getClass()) {
								Object reason = m.invoke(value);
								if (reason != null) {
									return formatted((Component) reason);
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
