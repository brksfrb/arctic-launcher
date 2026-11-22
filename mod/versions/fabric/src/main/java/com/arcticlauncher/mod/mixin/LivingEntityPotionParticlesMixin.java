package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.llamalad7.mixinextras.injector.v2.WrapWithCondition;
import net.minecraft.client.Minecraft;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.level.Level;
import net.minecraft.core.particles.ParticleOptions;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

@Mixin(LivingEntity.class)
public class LivingEntityPotionParticlesMixin {
	// Both normal and ambient status effects use this emission site.
	@WrapWithCondition(method = "tickEffects", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/level/Level;addParticle(Lnet/minecraft/core/particles/ParticleOptions;DDDDDD)V"))
	private boolean arctic$showPotionParticles(Level world, ParticleOptions particle,
			double x, double y, double z, double dx, double dy, double dz) {
		ClientConfig config = ArcticClient.config();
		return config == null || !config.hideOwnPotionParticles || (Object) this != Minecraft.getInstance().player;
	}
}
