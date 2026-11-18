package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyBanCommands;
import net.minecraft.server.command.CommandManager;
import net.minecraft.server.integrated.IntegratedServer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(IntegratedServer.class)
abstract class HostedBanCommandsMixin {
	@Inject(method = "createCommandManager", at = @At("RETURN"))
	private void arctic$register(CallbackInfoReturnable<CommandManager> cir) {
		LegacyBanCommands.register(cir.getReturnValue(), (IntegratedServer) (Object) this);
	}
}
