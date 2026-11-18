//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import net.minecraft.commands.CommandSource;
import net.minecraft.commands.CommandSourceStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(CommandSourceStack.class)
public interface CommandSourceStackAccess {
	@Accessor("source")
	CommandSource arctic$source();
}
//#endif
