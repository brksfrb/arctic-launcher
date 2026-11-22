package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.llamalad7.mixinextras.injector.v2.WrapWithCondition;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.LivingEntity;
import net.minecraft.world.World;
import net.minecraft.client.particle.ParticleType;
import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

@Mixin(LivingEntity.class)
public class LivingEntityPotionParticlesMixin {
	// Both normal and ambient status effects use this emission site.
	@WrapWithCondition(method = "tickStatusEffects", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/World;addParticle(Lnet/minecraft/client/particle/ParticleType;DDDDDD[I)V"))
	private boolean arctic$showPotionParticles(World world, ParticleType particle,
			double x, double y, double z, double dx, double dy, double dz, int[] params) {
		ClientConfig config = ArcticClient.config();
		Object entity = this;
		if (config == null || !(entity instanceof PlayerEntity)) {
			return true;
		}
		return entity == MinecraftClient.getInstance().player
				? !config.hideOwnPotionParticles
				: !config.hideOtherPlayersPotionParticles;
	}
}
