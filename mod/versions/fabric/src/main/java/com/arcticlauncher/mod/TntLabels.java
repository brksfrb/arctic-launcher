//#if MC >= 1.14
package com.arcticlauncher.mod;

import java.util.HashSet;
import java.util.Set;
import net.minecraft.client.Minecraft;
import net.minecraft.world.entity.Entity;

/**
 * TNT Timer: a lit TNT's name tag shows the seconds left on its fuse (the game draws custom names above
 * entities, near enough to read). Only on this client; the names are taken off again when it is switched off.
 */
final class TntLabels {
	private static final Set<Integer> NAMED = new HashSet<Integer>();

	private TntLabels() {}

	static void update(boolean on) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.level == null) {
			NAMED.clear();
			return;
		}
		if (!on && NAMED.isEmpty()) {
			return;
		}
		//#if MC >= 1.14
		for (Entity e : mc.level.entitiesForRendering()) {
		//#else
		for (Entity e : mc.level.loadedEntities) {
		//#endif
			if (!(e instanceof net.minecraft.world.entity.item.PrimedTnt)) {
				continue;
			}
			if (on) {
				int fuse = ((net.minecraft.world.entity.item.PrimedTnt) e).getFuse();
				String text = (fuse <= 20 ? "\u00a7c" : "\u00a7e") + (Math.round(fuse / 2.0) / 10.0) + "s";
				e.setCustomName(Compat.literal(text));
				e.setCustomNameVisible(true);
				NAMED.add(e.getId());
			} else if (NAMED.contains(e.getId())) {
				e.setCustomName(null);
				e.setCustomNameVisible(false);
			}
		}
		if (!on) {
			NAMED.clear();
		}
	}
}
//#endif
