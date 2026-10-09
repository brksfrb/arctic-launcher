package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Animation;
import com.arcticlauncher.client.looks.Cosmetics;
import com.arcticlauncher.client.looks.Rig;
import java.util.UUID;
import net.minecraft.client.model.HumanoidModel;
import net.minecraft.client.model.geom.ModelPart;
//#if MC >= 1.21.11
import net.minecraft.client.model.player.PlayerModel;
//#else
import net.minecraft.client.model.PlayerModel;
//#endif
//#if MC >= 1.21.9
import com.arcticlauncher.mod.cosmetic.CosmeticsLayer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
//#elif MC >= 1.21.2
import com.arcticlauncher.mod.cosmetic.AvatarIdentity;
import net.minecraft.client.renderer.entity.state.PlayerRenderState;
//#else
import net.minecraft.world.entity.LivingEntity;
//#endif
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Emotes pose the player: bones named like Blockbench's player template
 * (head, body, rightArm, leftArm, rightLeg, leftLeg) take the emote's
 * rotation instead of the walking pose, plus any position offset.
 */
@Mixin(PlayerModel.class)
abstract class PlayerModelEmoteMixin {
	private static final float DEG = (float) (Math.PI / 180);

	//#if MC >= 1.21.9
	@Inject(method = "setupAnim(Lnet/minecraft/client/renderer/entity/state/AvatarRenderState;)V", at = @At("TAIL"))
	private void arctic$emote(AvatarRenderState state, CallbackInfo ci) {
		arctic$pose(CosmeticsLayer.player(state));
	}
	//#elif MC >= 1.21.2
	@Inject(method = "setupAnim(Lnet/minecraft/client/renderer/entity/state/PlayerRenderState;)V", at = @At("TAIL"))
	private void arctic$emote(PlayerRenderState state, CallbackInfo ci) {
		arctic$pose(((AvatarIdentity) state).arctic$uuid());
	}
	//#else
	// Before 1.21.2 the model is posed from the entity, and the outer skin layers (sleeves, jacket,
	// trousers, hat) are copied from the parts at the end: they're copied again after the emote.
	@Shadow
	@Final
	public ModelPart leftSleeve;
	@Shadow
	@Final
	public ModelPart rightSleeve;
	@Shadow
	@Final
	public ModelPart leftPants;
	@Shadow
	@Final
	public ModelPart rightPants;
	@Shadow
	@Final
	public ModelPart jacket;

	/**
	 * What the expressive rig changed last frame, to put back first: before 1.21.2 the game doesn't
	 * reset every part each frame, so the rig's moves would add up. Null when nothing to undo.
	 */
	@Unique
	private float[][] arctic$undo;

	@Inject(method = "setupAnim(Lnet/minecraft/world/entity/LivingEntity;FFFFF)V", at = @At("HEAD"))
	private void arctic$undoRig(LivingEntity entity, float limbSwing, float limbSwingAmount, float age, float headYaw, float headPitch,
			CallbackInfo ci) {
		if (arctic$undo != null) {
			arctic$apply(arctic$parts(), arctic$undo);
			arctic$undo = null;
		}
	}

	@Inject(method = "setupAnim(Lnet/minecraft/world/entity/LivingEntity;FFFFF)V", at = @At("TAIL"))
	private void arctic$emote(LivingEntity entity, float limbSwing, float limbSwingAmount, float age, float headYaw, float headPitch,
			CallbackInfo ci) {
		if (arctic$pose(entity.getUUID())) {
			HumanoidModel<?> model = (HumanoidModel<?>) (Object) this;
			leftSleeve.copyFrom(model.leftArm);
			rightSleeve.copyFrom(model.rightArm);
			leftPants.copyFrom(model.leftLeg);
			rightPants.copyFrom(model.rightLeg);
			jacket.copyFrom(model.body);
			model.hat.copyFrom(model.head);
		}
	}
	//#endif

	/** Pose the parts for the emote this player is playing; whether one is playing. */
	private boolean arctic$pose(UUID id) {
		if (id == null || ArcticClient.looks() == null) {
			return false;
		}
		Cosmetics.Playing playing = ArcticClient.looks().cosmetics().playingFor(id);
		Animation animation = playing == null ? null : playing.emote.animation;
		if (animation == null) {
			return false;
		}
		float t = playing.seconds();
		HumanoidModel<?> model = (HumanoidModel<?>) (Object) this;
		if (Rig.expressive(animation)) {
			arctic$rig(animation, t);
			return true;
		}
		pose(animation, "head", model.head, t);
		pose(animation, "body", model.body, t);
		pose(animation, "rightArm", model.rightArm, t);
		pose(animation, "leftArm", model.leftArm, t);
		pose(animation, "rightLeg", model.rightLeg, t);
		pose(animation, "leftLeg", model.leftLeg, t);
		return true;
	}

	/** The expressive rig: the parts move together (see Rig). */
	private void arctic$rig(Animation animation, float t) {
		ModelPart[] parts = arctic$parts();
		float[][] before = arctic$read(parts);
		float[][] pivots = new float[Rig.PARTS][];
		float[][] rotations = new float[Rig.PARTS][];
		for (int i = 0; i < Rig.PARTS; i++) {
			pivots[i] = new float[] {before[i][0], before[i][1], before[i][2]};
			rotations[i] = new float[] {before[i][3], before[i][4], before[i][5]};
		}
		Rig.pose(animation, t, pivots, rotations);
		float[][] after = new float[Rig.PARTS][];
		for (int i = 0; i < Rig.PARTS; i++) {
			after[i] = new float[] {pivots[i][0], pivots[i][1], pivots[i][2], rotations[i][0], rotations[i][1], rotations[i][2]};
		}
		arctic$apply(parts, after);
		//#if MC < 1.21.2
		arctic$undo = before;
		//#endif
	}

	/** The six parts, in Rig's order. */
	@Unique
	private ModelPart[] arctic$parts() {
		HumanoidModel<?> model = (HumanoidModel<?>) (Object) this;
		return new ModelPart[] {model.head, model.body, model.rightArm, model.leftArm, model.rightLeg, model.leftLeg};
	}

	/** Each part's pivot and rotation: x, y, z, xRot, yRot, zRot. */
	@Unique
	private static float[][] arctic$read(ModelPart[] parts) {
		float[][] out = new float[parts.length][];
		for (int i = 0; i < parts.length; i++) {
			ModelPart p = parts[i];
			out[i] = new float[] {p.x, p.y, p.z, p.xRot, p.yRot, p.zRot};
		}
		return out;
	}

	@Unique
	private static void arctic$apply(ModelPart[] parts, float[][] values) {
		for (int i = 0; i < parts.length; i++) {
			ModelPart p = parts[i];
			float[] v = values[i];
			p.x = v[0];
			p.y = v[1];
			p.z = v[2];
			p.xRot = v[3];
			p.yRot = v[4];
			p.zRot = v[5];
		}
	}

	private static void pose(Animation animation, String bone, ModelPart part, float t) {
		float[] r = animation.rotation(bone, t);
		if (r != null) {
			part.xRot = r[0] * DEG;
			part.yRot = r[1] * DEG;
			part.zRot = r[2] * DEG;
		}
		float[] p = animation.position(bone, t);
		if (p != null) {
			part.x += p[0];
			part.y -= p[1];
			part.z += p[2];
		}
	}
}
