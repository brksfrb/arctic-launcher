package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Animation;
import com.arcticlauncher.client.looks.Cosmetics;
import net.minecraft.client.render.entity.model.EntityModel;
import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.render.model.ModelPart;
import net.minecraft.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
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
		pose(animation, "head", model.head, t);
		pose(animation, "body", model.body, t);
		pose(animation, "rightArm", model.rightArm, t);
		pose(animation, "leftArm", model.leftArm, t);
		pose(animation, "rightLeg", model.rightLeg, t);
		pose(animation, "leftLeg", model.leftLeg, t);
		EntityModel.copyModelPart(model.head, model.hat);
		EntityModel.copyModelPart(model.body, model.jacket);
		EntityModel.copyModelPart(model.rightArm, model.rightSleeve);
		EntityModel.copyModelPart(model.leftArm, model.leftSleeve);
		EntityModel.copyModelPart(model.rightLeg, model.rightPants);
		EntityModel.copyModelPart(model.leftLeg, model.leftPants);
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
