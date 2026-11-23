//#if MC >= 1.21.2
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ArmorVisibility;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.entity.HumanoidMobRenderer;
import net.minecraft.client.renderer.entity.state.HumanoidRenderState;
//#if MC >= 1.21.4
import net.minecraft.client.renderer.item.ItemModelResolver;
//#endif
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(HumanoidMobRenderer.class)
abstract class ArmorStateExtractionMixin {
	@Inject(method = "extractHumanoidRenderState", at = @At("TAIL"))
	private static void arctic$localPlayer(LivingEntity entity, HumanoidRenderState state, float partialTick,
			//#if MC >= 1.21.4
			ItemModelResolver resolver,
			//#endif
			CallbackInfo ci) {
		((ArmorVisibility) state).arctic$setLocalPlayer(entity == Minecraft.getInstance().player);
	}
}
//#endif
