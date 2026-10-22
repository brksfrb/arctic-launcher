package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.entity.LivingEntityRenderer;
import net.minecraft.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Own Name Tag: the game never draws your own name tag; in third person it is drawn above your head like anyone's. */
@Mixin(LivingEntityRenderer.class)
abstract class OwnNametagLegacyMixin {
	@Inject(method = "hasLabel(Lnet/minecraft/entity/LivingEntity;)Z", at = @At("RETURN"), cancellable = true)
	private void arctic$own(LivingEntity entity, CallbackInfoReturnable<Boolean> cir) {
		MinecraftClient client = MinecraftClient.getInstance();
		if (!cir.getReturnValueZ() && entity == client.player && ArcticClient.config() != null && ArcticClient.config().ownNametag
				&& client.options.perspective != 0) {
			cir.setReturnValue(true);
		}
	}
}
