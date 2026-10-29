package com.arcticlauncher.mod.mixin;

//#if MC >= 1.17
import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.Minecraft;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Team View (1.17 and newer): players on your scoreboard team get the game's glow outline, in their team color, through walls. */
@Mixin(Minecraft.class)
abstract class TeamViewClientMixin {
	@Inject(method = "shouldEntityAppearGlowing", at = @At("RETURN"), cancellable = true)
	private void arctic$teamView(Entity entity, CallbackInfoReturnable<Boolean> cir) {
		if (cir.getReturnValueZ() || ArcticClient.config() == null || !ArcticClient.config().teamView) {
			return;
		}
		Player self = ((Minecraft) (Object) this).player;
		if (self != null && entity instanceof Player && entity != self && entity.getTeam() != null && entity.getTeam() == self.getTeam()) {
			cir.setReturnValue(true);
		}
	}
}
//#endif
