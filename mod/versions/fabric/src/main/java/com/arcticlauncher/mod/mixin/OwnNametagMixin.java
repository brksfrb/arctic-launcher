//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.Compat;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Own Name Tag: the game never draws your own name tag; in third person it is drawn above your head like anyone's. */
@Mixin(LivingEntityRenderer.class)
abstract class OwnNametagMixin {
	//#if MC >= 1.21.2
	@Inject(method = "shouldShowName(Lnet/minecraft/world/entity/LivingEntity;D)Z", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$own(LivingEntity entity, double distance, CallbackInfoReturnable<Boolean> cir) {
	//#else
	@Inject(method = "shouldShowName(Lnet/minecraft/world/entity/LivingEntity;)Z", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$own(LivingEntity entity, CallbackInfoReturnable<Boolean> cir) {
	//#endif
		Minecraft mc = Minecraft.getInstance();
		if (!cir.getReturnValueZ() && entity == mc.player && ArcticClient.config() != null && ArcticClient.config().ownNametag
				&& Compat.camera() != 0) {
			cir.setReturnValue(true);
		}
	}
}
//#endif
