//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import net.minecraft.client.render.item.ItemRenderer;
import net.minecraft.client.render.model.BakedModel;
import net.minecraft.client.render.model.json.ModelTransformation;
import net.minecraft.client.render.model.json.Transformation;
import net.minecraft.entity.LivingEntity;
import com.arcticlauncher.legacy.LegacyOldItems;
import net.minecraft.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Vanilla camera hook for the shared 1.7 transforms, including replacement culling. */
@Mixin(ItemRenderer.class)
abstract class OldItemPositionMixin {
	private static final String WITH_MODEL = "renderItem(Lnet/minecraft/item/ItemStack;Lnet/minecraft/client/render/model/BakedModel;"
			+ "Lnet/minecraft/client/render/model/json/ModelTransformation$Mode;)V";

	/** What is being drawn (this frame's draw call on this thread). */
	@Unique
	private static ItemStack arctic$stack;
	@Unique
	private static BakedModel arctic$model;
	@Unique
	private static LivingEntity arctic$entity;
	/** The item's turn mirrors it (a negative size), so its faces must be culled the other way round. */
	@Unique
	private static int arctic$replacement;

	@Shadow
	private boolean isNegativeScale(Transformation transformation) {
		return false;
	}

	@Inject(method = "renderItem(Lnet/minecraft/item/ItemStack;Lnet/minecraft/entity/LivingEntity;"
			+ "Lnet/minecraft/client/render/model/json/ModelTransformation$Mode;)V", at = @At("HEAD"))
	private void arctic$entity(ItemStack stack, LivingEntity entity, ModelTransformation.Mode mode, CallbackInfo ci) {
		arctic$entity = entity;
	}

	@Inject(method = WITH_MODEL, at = @At("HEAD"))
	private void arctic$model(ItemStack stack, BakedModel model, ModelTransformation.Mode mode, CallbackInfo ci) {
		arctic$stack = stack;
		arctic$model = model;
		arctic$replacement = -1;
	}

	@Redirect(method = WITH_MODEL, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/render/item/ItemRenderer;isNegativeScale(Lnet/minecraft/client/render/model/json/Transformation;)Z"))
	private boolean arctic$culling(ItemRenderer self, Transformation transformation) {
		return arctic$replacement < 0 ? isNegativeScale(transformation) : arctic$replacement == 1;
	}

	@Redirect(method = WITH_MODEL, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/render/model/json/ModelTransformation;apply(Lnet/minecraft/client/render/model/json/ModelTransformation$Mode;)V"))
	private void arctic$place(ModelTransformation transformation, ModelTransformation.Mode mode) {
		arctic$replacement = LegacyOldItems.apply(arctic$stack, arctic$model, arctic$entity, mode);
		if (arctic$replacement < 0) transformation.apply(mode);
	}
}
//#endif
