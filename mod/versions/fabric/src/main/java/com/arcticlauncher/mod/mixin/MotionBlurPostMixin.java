package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.6 && MC < 26.1
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.mod.VulkanCompat;
import com.mojang.blaze3d.resource.CrossFrameResourcePool;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.LevelTargetBundle;
import net.minecraft.client.renderer.PostChain;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Motion blur as the game's own post effect on 1.21.6-1.21.11, used under VulkanMod (which runs
 * post effects with Vulkan but not OpenGL calls; elsewhere MotionBlurGL does it). The same effect
 * as 26.1-26.2, right before the game's own effect and the HUD.
 */
@Mixin(GameRenderer.class)
abstract class MotionBlurPostMixin {
	@Shadow
	@Final
	private Minecraft minecraft;

	@Shadow
	@Final
	private CrossFrameResourcePool resourcePool;

	@Inject(method = "render", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/LevelRenderer;doEntityOutline()V", shift = At.Shift.AFTER))
	private void arctic$blur(net.minecraft.client.DeltaTracker deltaTracker, boolean advanceGameTime, CallbackInfo ci) {
		ClientConfig config = ArcticClient.config();
		if (!VulkanCompat.postBlur() || config == null || !config.motionBlur || minecraft.level == null) {
			return;
		}
		int strength = Math.max(1, Math.min(4, config.motionBlurStrength));
		Identifier id = Identifier.fromNamespaceAndPath("arctic", "motion_blur_gl_" + strength);
		PostChain chain = minecraft.getShaderManager().getPostChain(id, LevelTargetBundle.MAIN_TARGETS);
		if (chain != null) {
			chain.process(minecraft.getMainRenderTarget(), resourcePool);
		}
	}
}
//#endif
