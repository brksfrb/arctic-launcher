//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.IntegratedBanCommands;
import net.minecraft.commands.Commands;
//#if MC >= 1.19
import net.minecraft.commands.CommandBuildContext;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Commands.class)
abstract class HostedBanCommandsMixin {
	@Inject(method = "<init>", at = @At("RETURN"))
	private void arctic$register(
			//#if MC >= 1.16
			Commands.CommandSelection selection,
			//#if MC >= 1.19
			CommandBuildContext context,
			//#endif
			//#else
			boolean dedicated,
			//#endif
			CallbackInfo ci) {
		//#if MC >= 1.16
		if (selection == Commands.CommandSelection.INTEGRATED) {
		//#else
		if (!dedicated) {
		//#endif
			IntegratedBanCommands.register(((Commands) (Object) this).getDispatcher());
		}
	}
}
//#endif
