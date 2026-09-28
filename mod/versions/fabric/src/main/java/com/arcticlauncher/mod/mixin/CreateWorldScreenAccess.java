//#if MC < 1.19.4
package com.arcticlauncher.mod.mixin;

import net.minecraft.client.gui.screens.worldselection.CreateWorldScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** The Create button's action (self-tests make worlds with it before 1.19.4). */
@Mixin(CreateWorldScreen.class)
public interface CreateWorldScreenAccess {
	@Invoker("onCreate")
	void arctic$create();
}
//#endif
