package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.KeptStates;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.client.renderer.entity.HumanoidMobRenderer;
import net.minecraft.client.renderer.entity.state.HumanoidRenderState;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** A kept state's armor (see {@link KeptStates}): its copies left as they are while the player wears the same, on the same tick. */
@Mixin(HumanoidMobRenderer.class)
abstract class HumanoidKeptArmorMixin {
	@WrapOperation(method = "extractHumanoidRenderState", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/entity/HumanoidMobRenderer;getEquipmentIfRenderable(Lnet/minecraft/world/entity/LivingEntity;Lnet/minecraft/world/entity/EquipmentSlot;)Lnet/minecraft/world/item/ItemStack;"))
	private static ItemStack polonium$keptArmor(LivingEntity entity, EquipmentSlot slot, Operation<ItemStack> copy,
			@Local(argsOnly = true) HumanoidRenderState state) {
		int index = switch (slot) {
			case HEAD -> 0;
			case CHEST -> 1;
			case LEGS -> 2;
			case FEET -> 3;
			default -> -1;
		};
		if (index < 0 || !KeptStates.sameCopy(state, KeptStates.ARMOR + index, entity.getItemBySlot(slot), entity.tickCount)) {
			return copy.call(entity, slot);
		}
		return switch (slot) {
			case HEAD -> state.headEquipment;
			case CHEST -> state.chestEquipment;
			case LEGS -> state.legsEquipment;
			default -> state.feetEquipment;
		};
	}
}
