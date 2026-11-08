//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.legacy.LegacyHooks;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.render.item.ItemRenderer;
import net.minecraft.client.render.model.BakedModel;
import net.minecraft.client.render.model.json.ModelTransformation;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.BlockItem;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.util.UseAction;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Forge flavour of OldItemPositionMixin (src/main): same 1.7 item position, hooked at Forge's
 * handleCameraTransforms call instead of the vanilla transform calls Forge removed.
 *
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
		ClientConfig config = ArcticClient.config();
		ItemStack stack = arctic$stack;
		BakedModel model = arctic$model;
		boolean first = mode == ModelTransformation.Mode.FIRST_PERSON && config.oldItemPosition;
		boolean third = mode == ModelTransformation.Mode.THIRD_PERSON && config.oldThirdPerson && LegacyHooks.heldLayerActive;
		if ((!first && !third) || stack == null || model == null || model.hasDepth() || model.isBuiltin()
				|| stack.getItem() instanceof BlockItem) {
			return mode;
		}
		Item item = stack.getItem();
		if (third) {
			// The bow's and the tools' turns flip the item (a negative size); other items' don't.
			if (item == Items.BOW || item.isHandheld()) {
				GlStateManager.cullFace(1028);
			}
			heldInThirdPerson(item, stack, arctic$entity);
		} else if (item.shouldRotate()) {
			// Fishing rods and carrots on a sticks are turned around in the hand.
			GlStateManager.rotate(180f, 0f, 1f, 0f);
		}
		flatItem();
		return ModelTransformation.Mode.NONE;
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
