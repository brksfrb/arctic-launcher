//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import net.minecraft.world.inventory.Slot;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A slot's number inside its own container (the game only exposes it as a method from 1.17). */
@Mixin(Slot.class)
public interface SlotAccess {
	@Accessor("slot")
	int arctic$containerSlot();

}
//#endif
