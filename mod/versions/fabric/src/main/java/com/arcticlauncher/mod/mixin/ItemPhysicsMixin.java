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
	/** Radians per tick a falling item turns over. */
	@Unique
	private static final float TUMBLE_SPEED = 0.35F;
	/** The game's hover above the ground for items in the air. */
	@Unique
	private static final float HOVER = 0.0625F;

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
		((ItemPhysicsState) state).arctic$setPlace(place);
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
		if (((ItemPhysicsState) state).arctic$place() == ItemPhysicsState.IN_AIR) {
			pose.translate(0F, -(float) box.minY + HOVER, 0F);
			arctic$turn(pose, Axis.XP.rotation(state.ageInTicks * TUMBLE_SPEED));
		} else if (depth <= FLAT_DEPTH) {
			// Turned face up, the model's depth is its height: the lowest copy of the stack rests on the block.
			float stackHalf = depth * STACK_SPACING * (state.count - 1) / 2F;
			pose.translate(0F, stackHalf - (float) box.minZ, 0F);
			arctic$turn(pose, Axis.XP.rotation((float) (-Math.PI / 2)));
		} else {
			pose.translate(0F, -(float) box.minY, 0F);
		}
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
