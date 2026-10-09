package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Animation;
import com.arcticlauncher.client.looks.Cosmetics;
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
		pose(animation, "head", model.head, t);
		pose(animation, "body", model.body, t);
		pose(animation, "rightArm", model.rightArm, t);
		pose(animation, "leftArm", model.leftArm, t);
		pose(animation, "rightLeg", model.rightLeg, t);
		pose(animation, "leftLeg", model.leftLeg, t);
		return true;
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
