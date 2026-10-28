//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.legacy.LegacyHooks;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.render.item.ItemRenderer;
import net.minecraft.client.render.model.BakedModel;
import net.minecraft.client.render.model.json.ModelTransformation;
import net.minecraft.client.render.model.json.Transformation;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.BlockItem;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.util.UseAction;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 1.7 item position, 1.8.9: flat items (tools, swords, food, bows) are held the way 1.7 held them, in
 * first person and in third person. 1.7 drew an item as a flat picture with one fixed turn and offset, and
 * set it into the hand with a few more per kind of item; 1.8 gave each item model its own. Both are
 * written out here from 1.7.10's code and swapped in for the model's own, with the model's frame
 * (centered, half size) taken into account.
 */
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
	private static boolean arctic$mirrored;

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
		arctic$mirrored = false;
	}

	@Redirect(method = WITH_MODEL, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/render/item/ItemRenderer;isNegativeScale(Lnet/minecraft/client/render/model/json/Transformation;)Z"))
	private boolean arctic$culling(ItemRenderer self, Transformation transformation) {
		return isNegativeScale(transformation) ^ arctic$mirrored;
	}

	@Redirect(method = WITH_MODEL, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/render/model/json/ModelTransformation;apply(Lnet/minecraft/client/render/model/json/ModelTransformation$Mode;)V"))
	private void arctic$place(ModelTransformation transformation, ModelTransformation.Mode mode) {
		ClientConfig config = ArcticClient.config();
		ItemStack stack = arctic$stack;
		BakedModel model = arctic$model;
		boolean first = mode == ModelTransformation.Mode.FIRST_PERSON && config.oldItemPosition;
		boolean third = mode == ModelTransformation.Mode.THIRD_PERSON && config.oldThirdPerson && LegacyHooks.heldLayerActive;
		if ((!first && !third) || stack == null || model == null || model.hasDepth() || model.isBuiltin()
				|| stack.getItem() instanceof BlockItem) {
			transformation.apply(mode);
			return;
		}
		Item item = stack.getItem();
		if (third) {
			// The bow's and the tools' turns flip the item (a negative size); other items' don't.
			arctic$mirrored = item == Items.BOW || item.isHandheld();
			heldInThirdPerson(item, stack, arctic$entity);
		} else if (item.shouldRotate()) {
			// Fishing rods and carrots on a sticks are turned around in the hand.
			GlStateManager.rotate(180f, 0f, 1f, 0f);
		}
		flatItem();
	}

	/** How 1.7 set an item into the hand in third person (RenderPlayer): by kind. */
	@Unique
	private static void heldInThirdPerson(Item item, ItemStack stack, LivingEntity holder) {
		if (item == Items.BOW) {
			GlStateManager.translate(0f, 0.125f, 0.3125f);
			GlStateManager.rotate(-20f, 0f, 1f, 0f);
			GlStateManager.scale(0.625f, -0.625f, 0.625f);
			GlStateManager.rotate(-100f, 1f, 0f, 0f);
			GlStateManager.rotate(45f, 0f, 1f, 0f);
		} else if (item.isHandheld()) {
			if (item.shouldRotate()) {
				GlStateManager.rotate(180f, 0f, 0f, 1f);
				GlStateManager.translate(0f, -0.125f, 0f);
			}
			if (holder instanceof PlayerEntity && ((PlayerEntity) holder).getItemUseTicks() > 0
					&& stack.getUseAction() == UseAction.BLOCK) {
				// Blocking with a sword: held across the body.
				GlStateManager.translate(0.05f, 0f, -0.1f);
				GlStateManager.rotate(-50f, 0f, 1f, 0f);
				GlStateManager.rotate(-10f, 1f, 0f, 0f);
				GlStateManager.rotate(-60f, 0f, 0f, 1f);
			}
			GlStateManager.translate(0f, 0.1875f, 0f);
			GlStateManager.scale(0.625f, -0.625f, 0.625f);
			GlStateManager.rotate(-100f, 1f, 0f, 0f);
			GlStateManager.rotate(45f, 0f, 1f, 0f);
		} else {
			GlStateManager.translate(0.25f, 0.1875f, -0.1875f);
			GlStateManager.scale(0.375f, 0.375f, 0.375f);
			GlStateManager.rotate(60f, 0f, 0f, 1f);
			GlStateManager.rotate(-90f, 1f, 0f, 0f);
			GlStateManager.rotate(20f, 0f, 0f, 1f);
		}
	}

	/**
	 * 1.7's flat item (ItemRenderer.renderItem): a picture from (0,0) to (1,1), 1/16 thick behind it, set
	 * with a fixed offset, size and turn. The model here is centered on (0.5, 0.5, 0.5) and drawn at half
	 * size, with its two faces 1/16 apart around the middle, so those are put back first.
	 */
	@Unique
	private static void flatItem() {
		GlStateManager.translate(0f, -0.3f, 0f);
		GlStateManager.scale(1.5f, 1.5f, 1.5f);
		GlStateManager.rotate(50f, 0f, 1f, 0f);
		GlStateManager.rotate(335f, 0f, 0f, 1f);
		GlStateManager.translate(-0.9375f, -0.0625f, 0f);
		GlStateManager.translate(0.5f, 0.5f, -0.03125f);
		GlStateManager.scale(2f, 2f, 2f);
	}
}
//#endif
