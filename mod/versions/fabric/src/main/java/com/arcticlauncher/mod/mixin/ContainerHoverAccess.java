//#if MC >= 1.21.6
package com.arcticlauncher.mod.mixin;

import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.world.inventory.Slot;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The slot under the cursor of any container screen (for the hook on recipe-book screens, which don't declare the field). */
@Mixin(AbstractContainerScreen.class)
public interface ContainerHoverAccess {
	@Accessor("hoveredSlot")
	Slot arctic$hoveredSlot();
}
//#endif
