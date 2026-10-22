//#if MC >= 1.14
package com.arcticlauncher.mod;

import com.arcticlauncher.mod.mixin.SlotAccess;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.inventory.Slot;

/** Which of the player's own inventory slots a container slot is. */
public final class InventorySlots {
	private InventorySlots() {}

	/** The number of {@code slot} in the player's inventory (0-8 hotbar … 40 off hand), or -1 in any other container. */
	public static int index(Slot slot) {
		return slot != null && slot.container instanceof Inventory ? ((SlotAccess) slot).arctic$containerSlot() : -1;
	}
}
//#endif
