package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Animation;
import com.arcticlauncher.client.looks.Cosmetics;
import com.arcticlauncher.mod.cosmetic.CosmeticsLayer;
import java.util.UUID;
import net.minecraft.client.model.HumanoidModel;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.model.player.PlayerModel;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import org.spongepowered.asm.mixin.Mixin;
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

	@Inject(method = "setupAnim(Lnet/minecraft/client/renderer/entity/state/AvatarRenderState;)V", at = @At("TAIL"))
	private void arctic$emote(AvatarRenderState state, CallbackInfo ci) {
		if (ArcticClient.looks() == null) {
			return;
		}
		UUID id = CosmeticsLayer.player(state);
		Cosmetics.Playing playing = id == null ? null : ArcticClient.looks().cosmetics().playingFor(id);
		Animation animation = playing == null ? null : playing.emote.animation;
		if (animation == null) {
			return;
		}
		float t = playing.seconds();
		HumanoidModel<?> model = (HumanoidModel<?>) (Object) this;
		pose(animation, "head", model.head, t);
		pose(animation, "body", model.body, t);
		pose(animation, "rightArm", model.rightArm, t);
		pose(animation, "leftArm", model.leftArm, t);
		pose(animation, "rightLeg", model.rightLeg, t);
		pose(animation, "leftLeg", model.leftLeg, t);
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
//#endif
