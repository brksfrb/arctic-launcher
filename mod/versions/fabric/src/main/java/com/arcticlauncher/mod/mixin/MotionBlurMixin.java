//#if MC >= 26.1
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import com.arcticlauncher.mod.MotionBlurUniforms;
//#if MC >= 26.3
import java.util.List;
import net.minecraft.client.renderer.PostChain;
//#else
import com.mojang.blaze3d.resource.CrossFrameResourcePool;
import net.minecraft.client.renderer.LevelTargetBundle;
import net.minecraft.client.renderer.PostChain;
//#endif

/**
 * Motion blur: a post effect from the Arctic Client pack (see mod/tools/motion_blur.py) that smears the
 * picture along the way it moved since the last frame, after the world and before the HUD, so the HUD and
 * menus stay sharp. The camera's turn goes into the effect's uniform buffer each frame (MotionBlurUniforms).
 */
@Mixin(GameRenderer.class)
abstract class MotionBlurMixin {
	@Shadow
	@Final
	private Minecraft minecraft;

	/** The effect for the chosen strength, or null with motion blur off. */
	@Unique
	private static Identifier arctic$blur() {
		ClientConfig config = ArcticClient.config();
		if (config == null || !config.motionBlur) {
			MotionBlurUniforms.reset();
			return null;
		}
		//#if MC >= 26.3
		String name = "motion_blur_smear";
		//#else
		String name = "motion_blur_smear_gl";
		//#endif
		return Identifier.fromNamespaceAndPath("arctic", name);
	}

	//#if MC >= 26.3
	@Shadow
	@Final
	private List<Identifier> requestedPostEffects;

	@Shadow
	@Final
	private List<PostChain> appliedPostEffects;

	/** 26.3 keeps a list of the effects this frame wants: motion blur is one more. */
	@Inject(method = "update", at = @At("TAIL"))
	private void arctic$requestBlur(net.minecraft.client.DeltaTracker deltaTracker, CallbackInfo ci) {
		Identifier blur = arctic$blur();
		if (blur != null && minecraft.player != null) {
			requestedPostEffects.add(blur);
		}
	}

	/** The effects are about to run: this frame's numbers go into the blur's buffer. */
	@Inject(method = "applyPostEffects", at = @At("HEAD"))
	private void arctic$blurNumbers(CallbackInfo ci) {
		Identifier blur = arctic$blur();
		if (blur == null || minecraft.player == null) {
			return;
		}
		boolean turning = MotionBlurUniforms.prepare((GameRenderer) (Object) this, minecraft);
		for (PostChain chain : appliedPostEffects) {
			if (blur.equals(chain.id())) {
				MotionBlurUniforms.write(chain, turning);
			}
		}
	}
	//#else
	@Shadow
	@Final
	private CrossFrameResourcePool resourcePool;

	/** Before 26.3 the game runs one effect of its own; motion blur runs right before it. */
	@Inject(method = "render", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/LevelRenderer;doEntityOutline()V", shift = At.Shift.AFTER))
	private void arctic$applyBlur(net.minecraft.client.DeltaTracker deltaTracker, boolean advanceGameTime, CallbackInfo ci) {
		Identifier blur = arctic$blur();
		if (blur == null) {
			return;
		}
		PostChain chain = minecraft.getShaderManager().getPostChain(blur, LevelTargetBundle.MAIN_TARGETS);
		// Not turning: nothing to smear, and the pass is skipped.
		if (chain != null && MotionBlurUniforms.prepare((GameRenderer) (Object) this, minecraft)) {
			MotionBlurUniforms.write(chain, true);
			//#if MC >= 26.2
			chain.process(((GameRenderer) (Object) this).mainRenderTarget(), resourcePool);
			//#else
			chain.process(minecraft.getMainRenderTarget(), resourcePool);
			//#endif
		}
	}
	//#endif
}
//#endif
