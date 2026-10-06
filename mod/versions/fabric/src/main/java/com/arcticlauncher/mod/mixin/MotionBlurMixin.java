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
//#if MC >= 26.3
import java.util.List;
//#else
import com.mojang.blaze3d.resource.CrossFrameResourcePool;
import net.minecraft.client.renderer.LevelTargetBundle;
import net.minecraft.client.renderer.PostChain;
//#endif

/**
 * Motion blur: a post effect from the Arctic Client pack (see mod/tools/motion_blur.py) that blends each
 * frame with the last, after the world and before the HUD, so the HUD and menus stay sharp.
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
			return null;
		}
		int strength = Math.max(1, Math.min(4, config.motionBlurStrength));
		//#if MC >= 26.3
		String name = "motion_blur_" + strength;
		//#else
		String name = "motion_blur_gl_" + strength;
		//#endif
		return Identifier.fromNamespaceAndPath("arctic", name);
	}

	//#if MC >= 26.3
	@Shadow
	@Final
	private List<Identifier> requestedPostEffects;

	/** 26.3 keeps a list of the effects this frame wants: motion blur is one more. */
	@Inject(method = "update", at = @At("TAIL"))
	private void arctic$requestBlur(net.minecraft.client.DeltaTracker deltaTracker, CallbackInfo ci) {
		Identifier blur = arctic$blur();
		if (blur != null && minecraft.player != null) {
			requestedPostEffects.add(blur);
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
		if (chain != null) {
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
