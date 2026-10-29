package com.arcticlauncher.mod.mixin;

//#if MC >= 1.13 && MC < 1.17
import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.Minecraft;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Team View (1.13 - 1.16): players on your scoreboard team get the game's glow outline, in their team color, through walls. */
@Mixin(Entity.class)
abstract class TeamViewEntityMixin {
	@Inject(method = "isGlowing", at = @At("RETURN"), cancellable = true)
	private void arctic$teamView(CallbackInfoReturnable<Boolean> cir) {
		if (cir.getReturnValueZ() || ArcticClient.config() == null || !ArcticClient.config().teamView) {
			return;
		}
		Player self = Minecraft.getInstance().player;
		Entity entity = (Entity) (Object) this;
		if (self != null && entity instanceof Player && entity != self && entity.getTeam() != null && entity.getTeam() == self.getTeam()) {
			cir.setReturnValue(true);
		}
	}
}
//#endif
