package com.arcticlauncher.mod;

import com.arcticlauncher.mod.compat.KeyCodes;
import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;

/**
 * The Arctic menu key as one of Minecraft's key mappings: Controls lists it
 * under "Arctic", options.txt keeps it, and Arctic's settings edit this same
 * mapping, so the two places always agree. Right Shift by default.
 */
public final class MenuKeyMapping {
	private static KeyMapping mapping;

	private MenuKeyMapping() {}

	private static synchronized KeyMapping mapping() {
		if (mapping == null) {
			//#if MC >= 1.21.9
			KeyMapping.Category category = KeyMapping.Category.register(Compat.id("arctic", "menu"));
			//#else
			String category = "key.categories.arctic";
			registerCategory(category);
			//#endif
			//#if MC >= 26.3
			mapping = new KeyMapping("key.arctic.menu", InputConstants.Type.KEYBOARD, KeyCodes.RIGHT_SHIFT, category);
			//#else
			mapping = new KeyMapping("key.arctic.menu", InputConstants.Type.KEYSYM, KeyCodes.RIGHT_SHIFT, category);
			//#endif
		}
		return mapping;
	}

	//#if MC < 1.21.9
	/** Gives the category a place in Controls' sort order, which Minecraft's constructor does not (compareTo would throw). */
	static void registerCategory(String category) {
		CategoryOrder.register(com.arcticlauncher.mod.mixin.KeyMappingAccess.arctic$categoryOrder(), category);
	}
	//#endif

	/** Minecraft's key bindings plus this one (before the options file is read, so it is read too). */
	public static KeyMapping[] with(KeyMapping[] all) {
		KeyMapping menu = mapping();
		for (KeyMapping k : all) {
			if (k == menu) {
				return all;
			}
		}
		KeyMapping[] more = java.util.Arrays.copyOf(all, all.length + 1);
		more[all.length] = menu;
		return more;
	}

	/** The bound key's Minecraft name ("key.keyboard.right.shift"). */
	public static String key() {
		return mapping().saveString();
	}

	/** Bind a key by its Minecraft name, and save the options. */
	public static void set(String name) {
		KeyMapping menu = mapping();
		if (menu.saveString().equals(name)) {
			return;
		}
		menu.setKey(InputConstants.getKey(name));
		KeyMapping.resetMapping();
		Minecraft.getInstance().options.save();
	}
}
