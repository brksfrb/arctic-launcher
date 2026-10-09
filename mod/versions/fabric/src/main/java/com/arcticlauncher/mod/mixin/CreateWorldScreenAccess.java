//#if MC < 1.19.4
package com.arcticlauncher.mod.mixin;

import net.minecraft.client.gui.screens.worldselection.CreateWorldScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;
import org.spongepowered.asm.mixin.gen.Invoker;

/** The Create World screen before 1.19.4, where self-tests and duels make worlds through it. */
@Mixin(CreateWorldScreen.class)
public interface CreateWorldScreenAccess {
	@Invoker("onCreate")
	void arctic$create();

	/** The world name box (duel worlds are named so the next duel can clear them away). */
	@Accessor("nameEdit")
	net.minecraft.client.gui.components.EditBox arctic$nameEdit();

	@Accessor("commands")
	void arctic$setCommands(boolean on);

	@Accessor("commandsChanged")
	void arctic$setCommandsChanged(boolean changed);
}
//#endif
