//#if MC < 1.15
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.mod.ItemPhysicsState;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.renderer.entity.ItemEntityRenderer;
import net.minecraft.client.resources.model.BakedModel;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.item.ItemEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Item physics on 1.14 (see ItemPhysicsMixin): a dropped item lies flat on the ground at its own random
 * angle (blocks stand on it), and tumbles while it falls. In water it bobs as usual.
 *
 * <p>The game's item placement ({@code setupBobbingItem}) lifts the item by its bob and turns it by its
 * spin; with item physics on, the lift drops the bob and the turn is the fixed angle plus lying down or
 * tumbling.
 */
@Mixin(ItemEntityRenderer.class)
abstract class ItemPhysicsOldMixin {
	/** Degrees per second a falling item turns over. */
	@Unique
	private static final float TUMBLE_DEGREES = 400f;
	/** How fast a landed item settles flat (per second; about a third of a second to settle). */
	@Unique
	private static final float SETTLE_RATE = 12f;
	/** Longest frame gap the motion steps over at once. */
	@Unique
	private static final float MAX_STEP_S = 0.1f;
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
	/** The item's tilt this frame (degrees around its sideways axis; -90 is lying face up). */
	@Unique
	private static float arctic$tilt;
	/** How far it's lifted onto the top of thin snow (which has no height to stand on). */
	@Unique
	private static float arctic$lift;

	@Inject(method = "setupBobbingItem", at = @At("HEAD"))
	private void arctic$start(ItemEntity item, double x, double y, double z, float tickDelta, BakedModel model,
			CallbackInfoReturnable<Integer> cir) {
		ClientConfig config = ArcticClient.config();
		boolean on = config != null && config.itemPhysics && !item.isInWater();
		// Held in the air by a server (a display): drawn as the game draws it.
		arctic$item = on && !com.arcticlauncher.client.items.HeldItem.held((ItemPhysicsState.Motion) item,
				item.onGround, item.isNoGravity(), item.y, System.nanoTime()) ? item : null;
		arctic$tickDelta = tickDelta;
		arctic$flat = !model.isGui3d();
		if (arctic$item != null) {
			arctic$tilt = arctic$moveTilt((ItemPhysicsState.Motion) item, !item.onGround, arctic$flat);
			arctic$lift = item.onGround ? arctic$snowLift(item) : 0f;
		}
	}

	/** A single layer of snow has no height at all to stand on: a lying item is lifted onto the snow's top, not hidden in it. */
	@Unique
	private static float arctic$snowLift(ItemEntity item) {
		net.minecraft.core.BlockPos pos = new net.minecraft.core.BlockPos(Mth.floor(item.x), Mth.floor(item.y), Mth.floor(item.z));
		net.minecraft.world.level.block.state.BlockState block = item.level.getBlockState(pos);
		if (!(block.getBlock() instanceof net.minecraft.world.level.block.SnowLayerBlock)) {
			return 0f;
		}
		double top = pos.getY() + block.getValue(net.minecraft.world.level.block.SnowLayerBlock.LAYERS) / 8.0;
		double lift = top - item.y;
		return lift > 0 && lift < 1 ? (float) lift : 0f;
	}

	/**
	 * Turning over while it falls, easing to its resting angle once it lands (flat items face up or
	 * down, whichever is nearer; blocks onto their nearest side) instead of snapping.
	 */
	@Unique
	private static float arctic$moveTilt(ItemPhysicsState.Motion motion, boolean falling, boolean flat) {
		long now = System.nanoTime();
		long last = motion.arctic$tiltAt();
		float step = last == 0 ? 0f : Math.min(MAX_STEP_S, (now - last) / 1e9f);
		float tilt = motion.arctic$tilt();
		if (last == 0 && !falling) {
			// Already lying there when first seen: no settling to watch.
			tilt = flat ? -90f : 0f;
		} else if (falling) {
			tilt = (tilt + TUMBLE_DEGREES * step) % 360f;
		} else {
			float rest = flat ? (Math.abs(Mth.wrapDegrees(tilt - 90f)) < Math.abs(Mth.wrapDegrees(tilt + 90f)) ? 90f : -90f)
					: Math.round(tilt / 90f) * 90f;
			tilt += Mth.wrapDegrees(rest - tilt) * (1f - (float) Math.exp(-SETTLE_RATE * step));
		}
		motion.arctic$setTilt(tilt, now);
		return tilt;
	}

	@Redirect(method = "setupBobbingItem",
			at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/GlStateManager;translatef(FFF)V", ordinal = 0))
	private void arctic$height(float x, float y, float z) {
		ItemEntity item = arctic$item;
		if (item != null) {
			y -= Mth.sin((item.tickCount + arctic$tickDelta) / 10.0F + item.bobOffs) * 0.1F + 0.1F;
			y += arctic$lift;
			if (arctic$flat) {
				// Standing up the sprite's half height holds it off the ground; lying down, its stack does.
				float radians = arctic$tilt * ((float) Math.PI / 180f);
				float lying = LYING_LIFT + COPY_SPACING * (arctic$copies(item) - 1);
				y += STANDING_LIFT * Math.abs(Mth.cos(radians)) + lying * Math.abs(Mth.sin(radians)) - STANDING_LIFT;
			}
		}
		GlStateManager.translatef(x, y, z);
	}

	@Redirect(method = "setupBobbingItem",
			at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/GlStateManager;rotatef(FFFF)V", ordinal = 0))
	private void arctic$angle(float angle, float axisX, float axisY, float axisZ) {
		ItemEntity item = arctic$item;
		if (item == null) {
			GlStateManager.rotatef(angle, axisX, axisY, axisZ);
			return;
		}
		GlStateManager.rotatef(item.bobOffs * (180F / (float) Math.PI), 0F, 1F, 0F);
		// Lying face up (-90): the copies of a stack, spread along the model's depth, pile up from the ground.
		GlStateManager.rotatef(arctic$tilt, 1F, 0F, 0F);
	}

	/** How many copies the game draws for this stack (as its own getRenderAmount). */
	@Unique
	private static int arctic$copies(ItemEntity item) {
		int count = item.getItem().getCount();
		return count > 48 ? 5 : count > 32 ? 4 : count > 16 ? 3 : count > 1 ? 2 : 1;
	}
}
//#endif
