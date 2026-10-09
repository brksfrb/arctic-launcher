package com.arcticlauncher.legacy.mixin;

import net.minecraft.scoreboard.Scoreboard;
import net.minecraft.scoreboard.ScoreboardObjective;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Servers (Hypixel) remove scoreboard objectives the client never had: Minecraft then throws a
 * NullPointerException for each one, hundreds per session. Removing nothing does nothing.
 */
@Mixin(Scoreboard.class)
abstract class ScoreboardRemoveMixin {
	@Inject(method = "removeObjective", at = @At("HEAD"), cancellable = true)
	private void arctic$nothingToRemove(ScoreboardObjective objective, CallbackInfo ci) {
		if (objective == null) {
			ci.cancel();
		}
	}
}
