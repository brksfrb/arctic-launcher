//#if MC >= 1.12
package com.arcticlauncher.legacy.mixin;

import net.minecraft.client.MinecraftClient;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** 1.12's toast pop-ups (advancements, recipes), so Arctic's go below them. */
@Mixin(MinecraftClient.class)
public interface ToastsAccess {
	@Accessor("field_15868")
	net.minecraft.class_3264 arctic$toasts();
}
//#endif
