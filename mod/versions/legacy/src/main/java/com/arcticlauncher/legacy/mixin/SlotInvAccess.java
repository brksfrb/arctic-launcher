package com.arcticlauncher.legacy.mixin;

import net.minecraft.inventory.slot.Slot;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A slot's number inside its own inventory. */
@Mixin(Slot.class)
public interface SlotInvAccess {
	@Accessor("invSlot")
	int arctic$invSlot();
}
