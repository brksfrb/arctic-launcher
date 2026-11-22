package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(LivingEntity.class)
public class LivingEntityPotionParticlesMixin {
	// Forge's Mixin 0.7 lacks MixinExtras. The spawn call is immediately followed by RETURN.
	@Inject(method = "tickStatusEffects", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/World;addParticle(Lnet/minecraft/client/particle/ParticleType;DDDDDD[I)V"),
			cancellable = true, require = 1)
	private void arctic$hidePotionParticles(CallbackInfo ci) {
		ClientConfig config = ArcticClient.config();
		if (config != null && config.hideOwnPotionParticles && (Object) this == MinecraftClient.getInstance().player) {
			ci.cancel();
		}
	}
}
