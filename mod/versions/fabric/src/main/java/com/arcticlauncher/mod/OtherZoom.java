//#if MC >= 1.14
package com.arcticlauncher.mod;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;

/** OptiFine's zoom key ("of.key.zoom"), taken off the key Arctic's zoom uses. */
final class OtherZoom {
	private static final String OPTIFINE_ZOOM = "of.key.zoom";
	private static final String UNBOUND = "key.keyboard.unknown";

	private OtherZoom() {}

	static boolean disable(Minecraft mc) {
		boolean changed = false;
		for (KeyMapping key : mc.options.keyMappings) {
			if (OPTIFINE_ZOOM.equals(key.getName()) && !UNBOUND.equals(key.saveString())) {
				//#if MC >= 1.21.2
				key.setKey(InputConstants.UNKNOWN);
				//#else
				mc.options.setKey(key, InputConstants.UNKNOWN);
				//#endif
				changed = true;
			}
		}
		if (changed) {
			KeyMapping.resetMapping();
		}
		return changed;
	}
}
//#endif
