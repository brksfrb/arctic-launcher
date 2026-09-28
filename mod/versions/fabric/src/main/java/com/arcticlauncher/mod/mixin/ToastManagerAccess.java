package com.arcticlauncher.mod.mixin;

//#if MC >= 1.19
//#if MC >= 1.21.3
import java.util.BitSet;

import net.minecraft.client.gui.components.toasts.ToastManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** Which toast slots Minecraft is using, so Arctic's pop-ups go below them. */
@Mixin(ToastManager.class)
public interface ToastManagerAccess {
	@Accessor("occupiedSlots")
	BitSet arctic$occupiedSlots();
}
//#else
// Before 1.21.3 the toast queue was ToastComponent, not ToastManager, but the
// occupiedSlots field has always had the same name.
import java.util.BitSet;

import net.minecraft.client.gui.components.toasts.ToastComponent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(ToastComponent.class)
public interface ToastManagerAccess {
	@Accessor("occupiedSlots")
	BitSet arctic$occupiedSlots();
}
//#endif
//#endif
