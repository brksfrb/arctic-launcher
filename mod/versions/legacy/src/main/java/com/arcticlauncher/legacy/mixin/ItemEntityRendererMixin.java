package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.render.entity.ItemEntityRenderer;
import net.minecraft.client.render.model.BakedModel;
import net.minecraft.entity.ItemEntity;
import net.minecraft.util.math.MathHelper;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Item physics: a dropped item lies flat on the ground at its own random angle (blocks stand on it), and
 * tumbles while it falls. In water it bobs as usual.
 *
 * <p>The game's item placement ({@code method_10221}) lifts the item by its bob and turns it by its spin;
 * with item physics on, the lift drops the bob and the turn is the fixed angle plus lying down or tumbling.
 */
@Mixin(ItemEntityRenderer.class)
abstract class ItemEntityRendererMixin {
	/** Degrees per tick a falling item turns over. */
	@Unique
	private static final float TUMBLE_DEGREES = 20f;
	/** The game lifts a flat item by a quarter of its ground scale (0.5) so it stands on the ground. */
	@Unique
	private static final float STANDING_LIFT = 0.125f;
	/** A flat item's height above the block it lies on, and per extra copy in its stack (half the game's spacing). */
	@Unique
	private static final float LYING_LIFT = 0.01f;
	@Unique
	private static final float COPY_SPACING = 0.0234375f;

	/** The item being placed with item physics (null: the game's own placement). Render thread only. */
	@Unique
	private static ItemEntity arctic$item;
	@Unique
	private static float arctic$tickDelta;
	@Unique
	private static boolean arctic$flat;

	@Inject(method = "method_10221", at = @At("HEAD"))
	private void arctic$start(ItemEntity item, double x, double y, double z, float tickDelta, BakedModel model,
			CallbackInfoReturnable<Integer> cir) {
		ClientConfig config = ArcticClient.config();
		arctic$item = config != null && config.itemPhysics && !item.isTouchingWater() ? item : null;
		arctic$tickDelta = tickDelta;
		arctic$flat = !model.hasDepth();
	}

	@Redirect(method = "method_10221",
			at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/GlStateManager;translate(FFF)V", ordinal = 0))
	private void arctic$height(float x, float y, float z) {
		ItemEntity item = arctic$item;
		if (item != null) {
			y -= MathHelper.sin((item.getAge() + arctic$tickDelta) / 10.0F + item.hoverHeight) * 0.1F + 0.1F;
			if (item.onGround && arctic$flat) {
				y += LYING_LIFT + COPY_SPACING * (arctic$copies(item) - 1) - STANDING_LIFT;
			}
		}
		GlStateManager.translate(x, y, z);
	}

	@Redirect(method = "method_10221",
			at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/GlStateManager;rotate(FFFF)V", ordinal = 0))
	private void arctic$angle(float angle, float axisX, float axisY, float axisZ) {
		ItemEntity item = arctic$item;
		if (item == null) {
			GlStateManager.rotate(angle, axisX, axisY, axisZ);
			return;
		}
		GlStateManager.rotate(item.hoverHeight * (180F / (float) Math.PI), 0F, 1F, 0F);
		if (!item.onGround) {
			GlStateManager.rotate((item.getAge() + arctic$tickDelta) * TUMBLE_DEGREES, 1F, 0F, 0F);
		} else if (arctic$flat) {
			// Face up; the copies of a stack, spread along the model's depth, pile up from the ground.
			GlStateManager.rotate(-90F, 1F, 0F, 0F);
		}
	}

	/** How many copies the game draws for this stack (as its own method_10222). */
	@Unique
	private static int arctic$copies(ItemEntity item) {
		//#if MC >= 1.11
		int count = item.getItemStack().getCount();
		//#else
		int count = item.getItemStack().count;
		//#endif
		return count > 48 ? 5 : count > 32 ? 4 : count > 16 ? 3 : count > 1 ? 2 : 1;
	}
}
