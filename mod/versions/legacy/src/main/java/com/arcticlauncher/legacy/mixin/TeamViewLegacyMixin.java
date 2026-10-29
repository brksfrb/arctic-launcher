package com.arcticlauncher.legacy.mixin;

//#if MC >= 1.9
import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.Entity;
import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Team View (1.9 - 1.12): players on your scoreboard team get the game's glow outline, in their team color, through walls. */
@Mixin(Entity.class)
abstract class TeamViewLegacyMixin {
	@Inject(method = "isGlowing", at = @At("RETURN"), cancellable = true)
	private void arctic$teamView(CallbackInfoReturnable<Boolean> cir) {
		if (cir.getReturnValueZ() || ArcticClient.config() == null || !ArcticClient.config().teamView) {
			return;
		}
		PlayerEntity self = MinecraftClient.getInstance().player;
		Entity entity = (Entity) (Object) this;
		if (self != null && entity instanceof PlayerEntity && entity != self && entity.getScoreboardTeam() != null
				&& entity.getScoreboardTeam() == self.getScoreboardTeam()) {
			cir.setReturnValue(true);
		}
	}
}
//#endif
