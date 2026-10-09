package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Animation;
import com.arcticlauncher.client.looks.Cosmetics;
import com.arcticlauncher.client.looks.Rig;
import net.minecraft.client.render.entity.model.EntityModel;
import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.render.model.ModelPart;
import net.minecraft.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Emotes pose the player (as on newer versions): bones named like Blockbench's player template take
 * the emote's rotation instead of the walking pose, plus any position offset; the outer skin layers
 * (sleeves, jacket, trousers, hat), copied from the parts at the end of setAngles, are copied again.
 * In these mappings a part's rotation is posX/posY/posZ and its pivot pivotX/pivotY/pivotZ.
 */
@Mixin(PlayerEntityModel.class)
abstract class PlayerModelEmoteMixin {
	private static final float DEG = (float) (Math.PI / 180);

	/**
	 * What the expressive rig changed last frame, to put back first: the game doesn't reset every
	 * part each frame, so the rig's moves would add up. Null when nothing to undo.
	 */
	@Unique
	private float[][] arctic$undo;

	@Inject(method = "setAngles(FFFFFFLnet/minecraft/entity/Entity;)V", at = @At("HEAD"))
	private void arctic$undoRig(float limbAngle, float limbDistance, float age, float headYaw, float headPitch, float scale, Entity entity,
			CallbackInfo ci) {
		if (arctic$undo != null) {
			apply(parts(), arctic$undo);
			arctic$undo = null;
		}
	}

	@Inject(method = "setAngles(FFFFFFLnet/minecraft/entity/Entity;)V", at = @At("TAIL"))
	private void arctic$emote(float limbAngle, float limbDistance, float age, float headYaw, float headPitch, float scale, Entity entity,
			CallbackInfo ci) {
		if (entity == null || ArcticClient.looks() == null) {
			return;
		}
		Cosmetics.Playing playing = ArcticClient.looks().cosmetics().playingFor(entity.getUuid());
		Animation animation = playing == null ? null : playing.emote.animation;
		if (animation == null) {
			return;
		}
		float t = playing.seconds();
		PlayerEntityModel model = (PlayerEntityModel) (Object) this;
		if (Rig.expressive(animation)) {
			arctic$undo = rig(animation, t, parts());
		} else {
			pose(animation, "head", model.head, t);
			pose(animation, "body", model.body, t);
			pose(animation, "rightArm", model.rightArm, t);
			pose(animation, "leftArm", model.leftArm, t);
			pose(animation, "rightLeg", model.rightLeg, t);
			pose(animation, "leftLeg", model.leftLeg, t);
		}
		EntityModel.copyModelPart(model.head, model.hat);
		EntityModel.copyModelPart(model.body, model.jacket);
		EntityModel.copyModelPart(model.rightArm, model.rightSleeve);
		EntityModel.copyModelPart(model.leftArm, model.leftSleeve);
		EntityModel.copyModelPart(model.rightLeg, model.rightPants);
		EntityModel.copyModelPart(model.leftLeg, model.leftPants);
	}

	/** The expressive rig: the parts (in Rig's order) move together (see Rig); returns what they were. */
	private static float[][] rig(Animation animation, float t, ModelPart[] parts) {
		float[][] before = new float[Rig.PARTS][];
		float[][] pivots = new float[Rig.PARTS][];
		float[][] rotations = new float[Rig.PARTS][];
		for (int i = 0; i < Rig.PARTS; i++) {
			ModelPart p = parts[i];
			before[i] = new float[] {p.pivotX, p.pivotY, p.pivotZ, p.posX, p.posY, p.posZ};
			pivots[i] = new float[] {p.pivotX, p.pivotY, p.pivotZ};
			rotations[i] = new float[] {p.posX, p.posY, p.posZ};
		}
		Rig.pose(animation, t, pivots, rotations);
		float[][] after = new float[Rig.PARTS][];
		for (int i = 0; i < Rig.PARTS; i++) {
			after[i] = new float[] {pivots[i][0], pivots[i][1], pivots[i][2], rotations[i][0], rotations[i][1], rotations[i][2]};
		}
		apply(parts, after);
		return before;
	}

	private ModelPart[] parts() {
		PlayerEntityModel model = (PlayerEntityModel) (Object) this;
		return new ModelPart[] {model.head, model.body, model.rightArm, model.leftArm, model.rightLeg, model.leftLeg};
	}

	/** Pivot and rotation of each part: pivotX, pivotY, pivotZ, posX, posY, posZ (rotations here). */
	private static void apply(ModelPart[] parts, float[][] values) {
		for (int i = 0; i < parts.length; i++) {
			ModelPart p = parts[i];
			float[] v = values[i];
			p.pivotX = v[0];
			p.pivotY = v[1];
			p.pivotZ = v[2];
			p.posX = v[3];
			p.posY = v[4];
			p.posZ = v[5];
		}
	}

	private static void pose(Animation animation, String bone, ModelPart part, float t) {
		float[] r = animation.rotation(bone, t);
		if (r != null) {
			part.posX = r[0] * DEG;
			part.posY = r[1] * DEG;
			part.posZ = r[2] * DEG;
		}
		float[] p = animation.position(bone, t);
		if (p != null) {
			part.pivotX += p[0];
			part.pivotY -= p[1];
			part.pivotZ += p[2];
		}
	}
}
