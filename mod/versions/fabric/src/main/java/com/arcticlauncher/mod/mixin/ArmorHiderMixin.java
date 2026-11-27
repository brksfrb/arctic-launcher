package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ArmorVisibility;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
//#if MC < 1.21.9
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
//#endif
//#if MC < 1.21.2
import net.minecraft.client.Minecraft;
import net.minecraft.world.entity.LivingEntity;
//#else
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.renderer.entity.state.HumanoidRenderState;
import net.minecraft.world.item.Items;
//#endif
//#if MC >= 1.21.9
import net.minecraft.client.renderer.SubmitNodeCollector;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#elif MC >= 1.21.2
import net.minecraft.client.model.HumanoidModel;
import net.minecraft.client.renderer.MultiBufferSource;
//#endif
//#if MC < 1.16
import net.minecraft.client.renderer.entity.layers.AbstractArmorLayer;
//#else
import net.minecraft.client.renderer.entity.layers.HumanoidArmorLayer;
//#endif

//#if MC < 1.16
@Mixin(AbstractArmorLayer.class)
//#else
@Mixin(HumanoidArmorLayer.class)
//#endif
abstract class ArmorHiderMixin {
	//#if MC >= 1.21.9
	@Inject(method = "renderArmorPiece", at = @At("HEAD"), cancellable = true)
	private void arctic$hide(PoseStack pose, SubmitNodeCollector collector, ItemStack stack, EquipmentSlot slot,
			int light, HumanoidRenderState state, CallbackInfo ci) {
		if (((ArmorVisibility) state).arctic$isLocalPlayer() && !stack.is(Items.ELYTRA) && ArmorVisibility.hides(slot)) {
			ci.cancel();
		}
	}
	//#elif MC >= 1.21.2
	@WrapOperation(method = "extractRenderState(Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/MultiBufferSource;ILnet/minecraft/client/renderer/entity/state/HumanoidRenderState;FF)V", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/entity/layers/HumanoidArmorLayer;renderArmorPiece(Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/world/item/ItemStack;Lnet/minecraft/world/entity/EquipmentSlot;ILnet/minecraft/client/model/HumanoidModel;)V"))
	private void arctic$hide(HumanoidArmorLayer<?, ?, ?> layer, PoseStack pose, MultiBufferSource buffers,
			ItemStack stack, EquipmentSlot slot, int light, HumanoidModel<?> model, Operation<Void> original,
			PoseStack renderPose, MultiBufferSource renderBuffers, int renderLight, HumanoidRenderState state,
			float yaw, float pitch) {
		if (!((ArmorVisibility) state).arctic$isLocalPlayer() || stack.is(Items.ELYTRA) || !ArmorVisibility.hides(slot)) {
			original.call(layer, pose, buffers, stack, slot, light, model);
		}
	}
	//#else
	@WrapOperation(method = "renderArmorPiece", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/entity/LivingEntity;getItemBySlot(Lnet/minecraft/world/entity/EquipmentSlot;)Lnet/minecraft/world/item/ItemStack;"))
	private ItemStack arctic$hide(LivingEntity entity, EquipmentSlot slot, Operation<ItemStack> original) {
		return entity == Minecraft.getInstance().player && ArmorVisibility.hides(slot)
				? ItemStack.EMPTY : original.call(entity, slot);
	}
	//#endif
}
