package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.gpu.Crowd;
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.layers.ItemInHandLayer;
import net.minecraft.client.renderer.entity.state.ArmedEntityRenderState;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Which hand an item is drawn into, for the crowd path's recording ({@link Crowd}). */
@Mixin(ItemInHandLayer.class)
abstract class ItemInHandLayerMixin {
	@Inject(method = "submitArmWithItem", at = @At("HEAD"))
	private void polonium$arm(ArmedEntityRenderState state, ItemStackRenderState item, ItemStack itemStack, HumanoidArm arm, PoseStack poseStack,
			SubmitNodeCollector collector, int lightCoords, CallbackInfo ci) {
		Crowd.arm(arm);
	}
}
