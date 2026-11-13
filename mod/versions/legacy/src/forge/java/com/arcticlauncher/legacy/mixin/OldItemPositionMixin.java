//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.render.item.ItemRenderer;
import net.minecraft.client.render.model.BakedModel;
import net.minecraft.client.render.model.json.ModelTransformation;
import net.minecraft.entity.LivingEntity;
import com.arcticlauncher.legacy.LegacyOldItems;
import net.minecraft.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Forge camera hook for the shared 1.7 transforms. The target is required. */
@Mixin(ItemRenderer.class)
abstract class OldItemPositionMixin {
	private static final String WITH_MODEL = "renderItem(Lnet/minecraft/item/ItemStack;Lnet/minecraft/client/render/model/BakedModel;"
			+ "Lnet/minecraft/client/render/model/json/ModelTransformation$Mode;)V";

	/**
	 * Forge's hook, in Forge's (SRG) names already: the remapper only translates calls on game classes it
	 * knows, and this one is Forge's own, so its game types in the descriptor are written out as the game
	 * has them under Forge (IBakedModel, ItemCameraTransforms$TransformType).
	 */
	private static final String FORGE_CAMERA_TRANSFORMS = "Lnet/minecraftforge/client/ForgeHooksClient;handleCameraTransforms("
			+ "Lnet/minecraft/client/resources/model/IBakedModel;"
			+ "Lnet/minecraft/client/renderer/block/model/ItemCameraTransforms$TransformType;)"
			+ "Lnet/minecraft/client/resources/model/IBakedModel;";

	/** What is being drawn (this frame's draw call on this thread). */
	@Unique
	private static ItemStack arctic$stack;
	@Unique
	private static BakedModel arctic$model;
	@Unique
	private static LivingEntity arctic$entity;
	@Inject(method = "renderItem(Lnet/minecraft/item/ItemStack;Lnet/minecraft/entity/LivingEntity;"
			+ "Lnet/minecraft/client/render/model/json/ModelTransformation$Mode;)V", at = @At("HEAD"))
	private void arctic$entity(ItemStack stack, LivingEntity entity, ModelTransformation.Mode mode, CallbackInfo ci) {
		arctic$entity = entity;
	}

	@Inject(method = WITH_MODEL, at = @At("HEAD"))
	private void arctic$model(ItemStack stack, BakedModel model, ModelTransformation.Mode mode, CallbackInfo ci) {
		arctic$stack = stack;
		arctic$model = model;
	}

	/**
	 * Forge's RenderItem has no vanilla transform calls left: it calls ForgeHooksClient.handleCameraTransforms
	 * (model, mode), which applies the model's own transform. The 1.7 position is set up here, just before
	 * that call, and the call is handed Mode.NONE (no transform) so it adds nothing on top. The culling flip
	 * of a mirrored item (vanilla's isNegativeScale step, which Forge dropped) is done here too; RenderItem
	 * puts the culling back after drawing.
	 */
	@ModifyArg(method = WITH_MODEL, index = 1, at = @At(value = "INVOKE",
			target = FORGE_CAMERA_TRANSFORMS))
	private ModelTransformation.Mode arctic$place(ModelTransformation.Mode mode) {
		int replacement = LegacyOldItems.apply(arctic$stack, arctic$model, arctic$entity, mode);
		if (replacement < 0) return mode;
		// Forge's generated ItemLayerModel uses identity for NONE, and still selects its
		// unculled perspective variant. Its vertices use the same [0,1] frame as vanilla.
		if (replacement == 1) GlStateManager.cullFace(1028);
		return ModelTransformation.Mode.NONE;
	}
}
//#endif
