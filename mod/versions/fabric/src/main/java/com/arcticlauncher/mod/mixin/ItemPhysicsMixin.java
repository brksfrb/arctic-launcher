//#if MC >= 26.1
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.mod.ItemPhysicsState;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.math.Axis;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.ItemEntityRenderer;
import net.minecraft.client.renderer.entity.state.ItemEntityRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.phys.AABB;
import org.joml.Quaternionf;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Item physics: a dropped item lies flat on the ground at its own random angle (blocks stand on it),
 * and tumbles while it falls. In water and lava it bobs as usual.
 *
 * <p>The game lifts the item by its bob and turns it by its spin; with item physics on the bob is dropped,
 * the spin is a fixed angle, and the pose is finished right before the item is drawn.
 */
@Mixin(ItemEntityRenderer.class)
abstract class ItemPhysicsMixin {
	/** Item models this thin or thinner are flat (sprites); thicker ones are blocks and 3D items. */
	@Unique
	private static final float FLAT_DEPTH = 0.0625F;
	/** Copies in a stack of flat items are this many model depths apart (as the game draws them). */
	@Unique
	private static final float STACK_SPACING = 1.5F;
	/** Radians per second a falling item turns over. */
	@Unique
	private static final float TUMBLE_SPEED = 7F;
	/** How fast a landed item settles flat (per second; about a third of a second to settle). */
	@Unique
	private static final float SETTLE_RATE = 12F;
	/** Longest frame gap the motion steps over at once (a stall or an item coming back into view). */
	@Unique
	private static final float MAX_STEP_S = 0.1F;
	@Unique
	private static final float HALF_PI = (float) (Math.PI / 2);

	@Unique
	private static boolean arctic$on() {
		ClientConfig config = ArcticClient.config();
		return config != null && config.itemPhysics;
	}

	@Unique
	private static boolean arctic$physics(ItemEntityRenderState state) {
		return arctic$on() && ((ItemPhysicsState) state).arctic$place() != ItemPhysicsState.FLOATING;
	}

	@Inject(method = "extractRenderState(Lnet/minecraft/world/entity/item/ItemEntity;Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;F)V",
			at = @At("TAIL"))
	private void arctic$notePlace(ItemEntity entity, ItemEntityRenderState state, float partialTicks, CallbackInfo ci) {
		int place = entity.isInLiquid() ? ItemPhysicsState.FLOATING
				: entity.onGround() ? ItemPhysicsState.ON_GROUND : ItemPhysicsState.IN_AIR;
		ItemPhysicsState physics = (ItemPhysicsState) state;
		physics.arctic$setPlace(place);
		if (place == ItemPhysicsState.FLOATING || !arctic$on() || state.item.isEmpty()) {
			return;
		}
		physics.arctic$setTilt(arctic$moveTilt((ItemPhysicsState.Motion) entity, place == ItemPhysicsState.IN_AIR,
				state.item.getModelBoundingBox().getZsize() <= FLAT_DEPTH));
	}

	/**
	 * The item's tilt this frame: turning over while it falls, easing to its resting angle once it lands
	 * (flat items face up or down, whichever is nearer; blocks onto their nearest side) instead of snapping.
	 */
	@Unique
	private static float arctic$moveTilt(ItemPhysicsState.Motion motion, boolean falling, boolean flat) {
		long now = System.nanoTime();
		long last = motion.arctic$tiltAt();
		float step = last == 0 ? 0F : Math.min(MAX_STEP_S, (now - last) / 1e9F);
		float tilt = motion.arctic$tilt();
		if (last == 0 && !falling) {
			// Already lying there when first seen (joined, or came back into view): no settling to watch.
			tilt = flat ? -HALF_PI : 0F;
		} else if (falling) {
			tilt = (tilt + TUMBLE_SPEED * step) % (float) (Math.PI * 2);
		} else {
			float rest = flat ? (Math.abs(arctic$wrap(tilt - HALF_PI)) < Math.abs(arctic$wrap(tilt + HALF_PI)) ? HALF_PI : -HALF_PI)
					: Math.round(tilt / HALF_PI) * HALF_PI;
			tilt += arctic$wrap(rest - tilt) * (1F - (float) Math.exp(-SETTLE_RATE * step));
		}
		motion.arctic$setTilt(tilt, now);
		return tilt;
	}

	/** An angle difference brought into -pi..pi (the short way round). */
	@Unique
	private static float arctic$wrap(float angle) {
		float twoPi = (float) (Math.PI * 2);
		angle %= twoPi;
		if (angle > Math.PI) {
			angle -= twoPi;
		} else if (angle < -Math.PI) {
			angle += twoPi;
		}
		return angle;
	}

	/** Whether the item being drawn uses item physics (render thread only; set as its drawing starts). */
	@Unique
	private static boolean arctic$drawing;

	@Inject(method = "submit(Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;Lnet/minecraft/client/renderer/state/level/CameraRenderState;)V",
			at = @At("HEAD"))
	private void arctic$start(ItemEntityRenderState state, PoseStack pose, SubmitNodeCollector nodes, CameraRenderState camera, CallbackInfo ci) {
		arctic$drawing = arctic$physics(state);
	}

	/** No bob: the height is set in {@link #arctic$pose}. */
	@ModifyArg(method = "submit(Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;Lnet/minecraft/client/renderer/state/level/CameraRenderState;)V",
			at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/vertex/PoseStack;translate(FFF)V"), index = 1)
	private float arctic$noBob(float y) {
		return arctic$drawing ? 0F : y;
	}

	/** No spin: each item keeps the random angle it was dropped with. */
	@Redirect(method = "submit(Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;Lnet/minecraft/client/renderer/state/level/CameraRenderState;)V",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/world/entity/item/ItemEntity;getSpin(FF)F"))
	private float arctic$fixedAngle(float age, float bobOffset) {
		return arctic$drawing ? bobOffset : ItemEntity.getSpin(age, bobOffset);
	}

	@Inject(method = "submit(Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;Lnet/minecraft/client/renderer/state/level/CameraRenderState;)V",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/renderer/entity/ItemEntityRenderer;submitMultipleFromCount(Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;ILnet/minecraft/client/renderer/entity/state/ItemClusterRenderState;Lnet/minecraft/util/RandomSource;Lnet/minecraft/world/phys/AABB;)V"))
	private void arctic$pose(ItemEntityRenderState state, PoseStack pose, SubmitNodeCollector nodes, CameraRenderState camera, CallbackInfo ci) {
		if (!arctic$drawing) {
			return;
		}
		AABB box = state.item.getModelBoundingBox();
		float depth = (float) box.getZsize();
		// Copies of a flat stack are spread along the model's depth (as the game draws them).
		float stackHalf = depth <= FLAT_DEPTH ? depth * STACK_SPACING * (state.count - 1) / 2F : 0F;
		float tilt = ((ItemPhysicsState) state).arctic$tilt();
		// Lifted so the lowest point of the turned model (stack included) touches the ground, at any tilt.
		float cos = (float) Math.cos(tilt);
		float sin = (float) Math.sin(tilt);
		float lowest = Math.min((float) box.minY * cos, (float) box.maxY * cos)
				+ Math.min(-((float) box.minZ - stackHalf) * sin, -((float) box.maxZ + stackHalf) * sin);
		pose.translate(0F, -lowest, 0F);
		arctic$turn(pose, Axis.XP.rotation(tilt));
	}

	@Unique
	private static void arctic$turn(PoseStack pose, Quaternionf rotation) {
		//#if MC >= 26.3
		pose.rotate(rotation);
		//#else
		pose.mulPose(rotation);
		//#endif
	}
}
//#endif
