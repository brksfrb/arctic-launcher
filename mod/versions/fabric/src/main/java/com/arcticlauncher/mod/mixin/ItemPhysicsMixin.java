package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.mod.ItemPhysicsState;
//#if MC < 1.21.2
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
//#endif
import com.mojang.blaze3d.vertex.PoseStack;
//#if MC >= 1.19.3
import com.mojang.math.Axis;
//#else
import com.mojang.math.Vector3f;
//#endif
//#if MC < 1.21.9
import net.minecraft.client.renderer.MultiBufferSource;
//#else
import net.minecraft.client.renderer.SubmitNodeCollector;
//#endif
import net.minecraft.client.renderer.entity.ItemEntityRenderer;
//#if MC >= 1.21.2
import net.minecraft.client.renderer.entity.state.ItemEntityRenderState;
//#endif
//#if MC >= 26.1
import net.minecraft.client.renderer.state.level.CameraRenderState;
//#elif MC >= 1.21.9
import net.minecraft.client.renderer.state.CameraRenderState;
//#endif
//#if MC < 1.21.2
import net.minecraft.client.resources.model.BakedModel;
//#endif
import net.minecraft.world.entity.item.ItemEntity;
//#if MC >= 1.21.5
import net.minecraft.world.phys.AABB;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
//#if MC >= 1.21.2
import org.spongepowered.asm.mixin.injection.Redirect;
//#endif
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Item physics: a dropped item lies flat on the ground at its own random angle (blocks stand on it),
 * and tumbles while it falls. In water and lava it bobs as usual.
 *
 * <p>The game lifts the item by its bob and turns it by its spin; with item physics on the bob is dropped,
 * the spin is a fixed angle, and the pose is finished right before the item is drawn.
 *
 * <p>The renderer comes in three shapes: up to 1.21.1 it draws straight from the entity (the item's place
 * and tilt are worked out as it draws), from 1.21.2 it draws from a render state that already holds them,
 * and from 1.21.9 that drawing is a {@code submit} (as in 26.x). Where the model can't be measured
 * (before 1.21.5) the ground pose's known shapes stand in: a sprite is a thin slab, a block a small cube.
 */
@Mixin(ItemEntityRenderer.class)
abstract class ItemPhysicsMixin {
	/** The method that lifts, turns and draws the item. */
	//#if MC >= 26.1
	@Unique
	private static final String DRAW = "submit(Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;Lnet/minecraft/client/renderer/state/level/CameraRenderState;)V";
	//#elif MC >= 1.21.9
	@Unique
	private static final String DRAW = "submit(Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;Lnet/minecraft/client/renderer/state/CameraRenderState;)V";
	//#elif MC >= 1.21.2
	@Unique
	private static final String DRAW = "render(Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/MultiBufferSource;I)V";
	//#else
	@Unique
	private static final String DRAW = "render(Lnet/minecraft/world/entity/item/ItemEntity;FFLcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/MultiBufferSource;I)V";
	//#endif
	/** The lift: a translate whose height is the bob (floats from 1.19.3). */
	//#if MC >= 1.19.3
	@Unique
	private static final String LIFT = "Lcom/mojang/blaze3d/vertex/PoseStack;translate(FFF)V";
	//#else
	@Unique
	private static final String LIFT = "Lcom/mojang/blaze3d/vertex/PoseStack;translate(DDD)V";
	//#endif
	/** The call that works out the angle about the up axis (the game's spin). */
	//#if MC >= 1.21.2
	@Unique
	private static final String SPIN = "Lnet/minecraft/world/entity/item/ItemEntity;getSpin(FF)F";
	//#elif MC >= 1.19.3
	@Unique
	private static final String SPIN = "Lcom/mojang/math/Axis;rotation(F)Lorg/joml/Quaternionf;";
	//#else
	@Unique
	private static final String SPIN = "Lcom/mojang/math/Vector3f;rotation(F)Lcom/mojang/math/Quaternion;";
	//#endif
	/** The call that applies that angle: the item's pose is finished right after it (before 1.21.9) or before the copies are drawn. */
	//#if MC >= 1.21.9
	@Unique
	private static final String TURN = "Lnet/minecraft/client/renderer/entity/ItemEntityRenderer;submitMultipleFromCount(Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/renderer/SubmitNodeCollector;ILnet/minecraft/client/renderer/entity/state/ItemClusterRenderState;Lnet/minecraft/util/RandomSource;Lnet/minecraft/world/phys/AABB;)V";
	//#elif MC >= 1.21.5
	@Unique
	private static final String TURN = "Lcom/mojang/blaze3d/vertex/PoseStack;mulPose(Lorg/joml/Quaternionfc;)V";
	//#elif MC >= 1.19.3
	@Unique
	private static final String TURN = "Lcom/mojang/blaze3d/vertex/PoseStack;mulPose(Lorg/joml/Quaternionf;)V";
	//#else
	@Unique
	private static final String TURN = "Lcom/mojang/blaze3d/vertex/PoseStack;mulPose(Lcom/mojang/math/Quaternion;)V";
	//#endif

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
	/** Where the ground pose puts a sprite item: height range, and how thick it is (before the model could be measured). */
	@Unique
	private static final float SPRITE_MIN_Y = -0.125F;
	@Unique
	private static final float SPRITE_MAX_Y = 0.375F;
	@Unique
	private static final float SPRITE_DEPTH = 0.03125F;
	/** The same for a block: a cube of this half size, centred this high. */
	@Unique
	private static final float BLOCK_MIN_Y = 0.0625F;
	@Unique
	private static final float BLOCK_MAX_Y = 0.3125F;
	@Unique
	private static final float BLOCK_HALF_DEPTH = 0.125F;

	@Unique
	private static boolean arctic$on() {
		ClientConfig config = ArcticClient.config();
		return config != null && config.itemPhysics;
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

	/**
	 * How far down the lowest point of the model reaches once turned by {@code tilt} around the sideways axis
	 * (the box is in the ground pose; a stack of flat items is spread {@code stackHalf} either way along z).
	 */
	@Unique
	private static float arctic$lowestPoint(float minY, float maxY, float minZ, float maxZ, float stackHalf, float tilt) {
		float cos = (float) Math.cos(tilt);
		float sin = (float) Math.sin(tilt);
		return Math.min(minY * cos, maxY * cos)
				+ Math.min(-(minZ - stackHalf) * sin, -(maxZ + stackHalf) * sin);
	}

	/** Turns the item over around its sideways axis. */
	@Unique
	private static void arctic$tiltBy(PoseStack pose, float tilt) {
		//#if MC >= 26.3
		pose.rotate(Axis.XP.rotation(tilt));
		//#elif MC >= 1.19.3
		pose.mulPose(Axis.XP.rotation(tilt));
		//#else
		pose.mulPose(Vector3f.XP.rotation(tilt));
		//#endif
	}

	//#if MC < 1.21.5
	/** {@link #arctic$lowestPoint} from the known shapes of the ground pose (a sprite item or a block). */
	@Unique
	private static float arctic$groundLowest(boolean flat, int copies, float tilt) {
		if (flat) {
			float stackHalf = SPRITE_DEPTH * STACK_SPACING * (copies - 1) / 2F;
			return arctic$lowestPoint(SPRITE_MIN_Y, SPRITE_MAX_Y, -SPRITE_DEPTH / 2F, SPRITE_DEPTH / 2F, stackHalf, tilt);
		}
		return arctic$lowestPoint(BLOCK_MIN_Y, BLOCK_MAX_Y, -BLOCK_HALF_DEPTH, BLOCK_HALF_DEPTH, 0F, tilt);
	}

	/** How many copies the game draws for a stack of this size. */
	@Unique
	private static int arctic$renderedAmount(int count) {
		return count > 48 ? 5 : count > 32 ? 4 : count > 16 ? 3 : count > 1 ? 2 : 1;
	}
	//#endif

//#if MC >= 1.21.2
	@Unique
	private static boolean arctic$physics(ItemEntityRenderState state) {
		return arctic$on() && ((ItemPhysicsState) state).arctic$place() != ItemPhysicsState.FLOATING;
	}

	// Written in two pieces: the build renames extractRenderState to render for the GUI classes of versions before 26.1,
	// which is not what the item renderer calls it (it has been extractRenderState since 1.21.2).
	@Inject(method = "extract" + "RenderState(Lnet/minecraft/world/entity/item/ItemEntity;Lnet/minecraft/client/renderer/entity/state/ItemEntityRenderState;F)V",
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
				arctic$flat(state)));
	}

	/** Whether the item's model is flat (a sprite) rather than a block or a 3D item. */
	@Unique
	private static boolean arctic$flat(ItemEntityRenderState state) {
		//#if MC >= 1.21.6
		return state.item.getModelBoundingBox().getZsize() <= FLAT_DEPTH;
		//#elif MC >= 1.21.5
		return arctic$measure(state).getZsize() <= FLAT_DEPTH;
		//#elif MC >= 1.21.4
		return !state.item.isGui3d();
		//#else
		return state.itemModel != null && !state.itemModel.isGui3d();
		//#endif
	}

	//#if MC >= 1.21.5 && MC < 1.21.6
	/** The model's box in the ground pose (1.21.5 measures it as the game does; later versions keep it on the model). */
	@Unique
	private static AABB arctic$measure(ItemEntityRenderState state) {
		AABB.Builder box = new AABB.Builder();
		state.item.visitExtents(box::include);
		return box.build();
	}
	//#endif

	/** How far down the item's model reaches in the ground pose, once turned by {@code tilt}. */
	@Unique
	private static float arctic$lowest(ItemEntityRenderState state, float tilt) {
		//#if MC >= 1.21.5
		//#if MC >= 1.21.6
		AABB box = state.item.getModelBoundingBox();
		//#else
		AABB box = arctic$measure(state);
		//#endif
		float depth = (float) box.getZsize();
		// Copies of a flat stack are spread along the model's depth (as the game draws them).
		float stackHalf = depth <= FLAT_DEPTH ? depth * STACK_SPACING * (state.count - 1) / 2F : 0F;
		return arctic$lowestPoint((float) box.minY, (float) box.maxY, (float) box.minZ, (float) box.maxZ, stackHalf, tilt);
		//#elif MC >= 1.21.4
		return arctic$groundLowest(arctic$flat(state), state.count, tilt);
		//#else
		return arctic$groundLowest(arctic$flat(state), arctic$renderedAmount(state.item.getCount()), tilt);
		//#endif
	}

	//#if MC >= 1.21.9
	@Inject(method = DRAW, at = @At("HEAD"))
	private void arctic$start(ItemEntityRenderState state, PoseStack pose, SubmitNodeCollector nodes, CameraRenderState camera, CallbackInfo ci) {
	//#else
	@Inject(method = DRAW, at = @At("HEAD"))
	private void arctic$start(ItemEntityRenderState state, PoseStack pose, MultiBufferSource buffers, int light, CallbackInfo ci) {
	//#endif
		arctic$drawing = arctic$physics(state);
	}

	/** No bob: the height is set in {@link #arctic$pose}. */
	@ModifyArg(method = DRAW, at = @At(value = "INVOKE", target = LIFT), index = 1)
	private float arctic$noBob(float y) {
		return arctic$drawing ? 0F : y;
	}

	/** No spin: each item keeps the random angle it was dropped with. */
	@Redirect(method = DRAW, at = @At(value = "INVOKE", target = SPIN))
	private float arctic$fixedAngle(float age, float bobOffset) {
		return arctic$drawing ? bobOffset : ItemEntity.getSpin(age, bobOffset);
	}

	//#if MC >= 1.21.9
	@Inject(method = DRAW, at = @At(value = "INVOKE", target = TURN))
	private void arctic$pose(ItemEntityRenderState state, PoseStack pose, SubmitNodeCollector nodes, CameraRenderState camera, CallbackInfo ci) {
	//#else
	// Right after the turn about the up axis (the only one), which is just before the copies are drawn.
	@Inject(method = DRAW, at = @At(value = "INVOKE", target = TURN, ordinal = 0, shift = At.Shift.AFTER))
	private void arctic$pose(ItemEntityRenderState state, PoseStack pose, MultiBufferSource buffers, int light, CallbackInfo ci) {
	//#endif
		if (!arctic$drawing) {
			return;
		}
		float tilt = ((ItemPhysicsState) state).arctic$tilt();
		// Lifted so the lowest point of the turned model (stack included) touches the ground, at any tilt.
		pose.translate(0F, -arctic$lowest(state, tilt), 0F);
		arctic$tiltBy(pose, tilt);
	}
//#else
	/** Whether the item being drawn is a flat sprite rather than a block (known once the game has its model). */
	@Unique
	private static boolean arctic$flat;
	/** The item's own fixed angle (the game's per-item random offset), in place of its spin. */
	@Unique
	private static float arctic$angle;

	@Inject(method = DRAW, at = @At("HEAD"))
	private void arctic$start(ItemEntity entity, float yaw, float partialTicks, PoseStack pose, MultiBufferSource buffers, int light, CallbackInfo ci) {
		// In water and lava it bobs as usual.
		arctic$drawing = arctic$on() && !entity.getItem().isEmpty() && !entity.isInWater() && !entity.isInLava();
		arctic$angle = entity.bobOffs;
	}

	@WrapOperation(method = DRAW, at = @At(value = "INVOKE", target = "Lnet/minecraft/client/resources/model/BakedModel;isGui3d()Z"))
	private boolean arctic$noteShape(BakedModel model, Operation<Boolean> original) {
		boolean gui3d = original.call(model);
		arctic$flat = !gui3d;
		return gui3d;
	}

	/** No bob: the height is set in {@link #arctic$pose}. */
	//#if MC >= 1.19.3
	@ModifyArg(method = DRAW, at = @At(value = "INVOKE", target = LIFT, ordinal = 0), index = 1)
	private float arctic$noBob(float y) {
		return arctic$drawing ? 0F : y;
	}
	//#else
	@ModifyArg(method = DRAW, at = @At(value = "INVOKE", target = LIFT, ordinal = 0), index = 1)
	private double arctic$noBob(double y) {
		return arctic$drawing ? 0D : y;
	}
	//#endif

	/** No spin: each item keeps the random angle it was dropped with. */
	@ModifyArg(method = DRAW, at = @At(value = "INVOKE", target = SPIN, ordinal = 0), index = 0)
	private float arctic$fixedAngle(float spin) {
		return arctic$drawing ? arctic$angle : spin;
	}

	// Right after the turn about the up axis (the only one), which is just before the copies are drawn.
	@Inject(method = DRAW, at = @At(value = "INVOKE", target = TURN, ordinal = 0, shift = At.Shift.AFTER))
	private void arctic$pose(ItemEntity entity, float yaw, float partialTicks, PoseStack pose, MultiBufferSource buffers, int light, CallbackInfo ci) {
		if (!arctic$drawing) {
			return;
		}
		float tilt = arctic$moveTilt((ItemPhysicsState.Motion) entity, !arctic$grounded(entity), arctic$flat);
		// Lifted so the lowest point of the turned model (stack included) touches the ground, at any tilt.
		pose.translate(0F, -arctic$groundLowest(arctic$flat, arctic$renderedAmount(entity.getItem().getCount()), tilt), 0F);
		arctic$tiltBy(pose, tilt);
	}

	@Unique
	private static boolean arctic$grounded(ItemEntity entity) {
		//#if MC >= 1.20
		return entity.onGround();
		//#elif MC >= 1.16
		return entity.isOnGround();
		//#else
		return entity.onGround;
		//#endif
	}
//#endif
}
