//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
import com.arcticlauncher.client.ArcticClient;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.render.entity.feature.HeldItemRenderer;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Marks the held-item layer that people and mobs wear (not a witch's potion, which sets its own turn): 1.7's
 * way of holding an item in third person (OldItemPositionMixin) is only for what this layer draws.
 */
@Mixin(HeldItemRenderer.class)
abstract class OldHeldLayerMixin {
	@Unique private LivingEntity arctic$holder;

	/** RenderPlayer in 1.7 used a stick for the cast rod, before choosing the tool pose. */
	@ModifyArg(method = "render", index = 1, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/render/item/HeldItemRenderer;renderItem(Lnet/minecraft/entity/LivingEntity;Lnet/minecraft/item/ItemStack;Lnet/minecraft/client/render/model/json/ModelTransformation$Mode;)V"))
	private ItemStack arctic$castRod(ItemStack stack) {
		if (ArcticClient.config().oldThirdPerson && arctic$holder instanceof PlayerEntity
				&& ((PlayerEntity) arctic$holder).fishHook != null && stack.getItem() == Items.FISHING_ROD) {
			return new ItemStack(Items.STICK);
		}
		return stack;
	}

	/** 1.7 attached the item directly to the arm; 1.8 adds this layer-only sneak offset. */
	@Redirect(method = "render", at = @At(value = "INVOKE",
			target = "Lcom/mojang/blaze3d/platform/GlStateManager;translate(FFF)V"))
	private void arctic$translation(float x, float y, float z) {
		if (ArcticClient.config().oldThirdPerson && x == 0f && y == 0.203125f && z == 0f) return;
		GlStateManager.translate(x, y, z);
	}

	@Inject(method = "render", at = @At("HEAD"))
	private void arctic$begin(LivingEntity entity, float a, float b, float c, float d, float e, float f, float g, CallbackInfo ci) {
		arctic$holder = entity;
		LegacyHooks.heldLayerActive = true;
	}

	@Inject(method = "render", at = @At("RETURN"))
	private void arctic$end(LivingEntity entity, float a, float b, float c, float d, float e, float f, float g, CallbackInfo ci) {
		LegacyHooks.heldLayerActive = false;
		arctic$holder = null;
	}
}
//#endif
