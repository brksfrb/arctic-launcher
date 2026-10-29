package com.arcticlauncher.mod.mixin;

//#if MC >= 1.13 && MC < 1.21.5
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
//#if MC >= 1.15
import com.mojang.blaze3d.systems.RenderSystem;
//#else
import com.mojang.blaze3d.platform.GlStateManager;
//#endif
//#if MC >= 1.21.2
import net.minecraft.client.renderer.FogParameters;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Fog: the distance where the fog starts and ends is stretched by the player's factor (or pushed out of sight).
 * 1.21.5 and newer upload the fog to the GPU as a buffer: see FogBufferMixin.
 */
//#if MC >= 1.15
@Mixin(RenderSystem.class)
//#else
@Mixin(GlStateManager.class)
//#endif
abstract class FogMixin {
	//#if MC >= 1.21.2
	@ModifyVariable(method = "setShaderFog", at = @At("HEAD"), argsOnly = true)
	private static FogParameters arctic$fog(FogParameters fog) {
		float f = factor();
		return f == 1f || fog == null ? fog : new FogParameters(fog.start() * f, fog.end() * f, fog.shape(), fog.red(), fog.green(), fog.blue(), fog.alpha());
	}
	//#elif MC >= 1.17
	@ModifyVariable(method = "setShaderFogStart", at = @At("HEAD"), argsOnly = true)
	private static float arctic$start(float start) {
		return start * factor();
	}

	@ModifyVariable(method = "setShaderFogEnd", at = @At("HEAD"), argsOnly = true)
	private static float arctic$end(float end) {
		return end * factor();
	}
	//#else
	@ModifyVariable(method = "fogStart", at = @At("HEAD"), argsOnly = true)
	private static float arctic$start(float start) {
		return start * factor();
	}

	@ModifyVariable(method = "fogEnd", at = @At("HEAD"), argsOnly = true)
	private static float arctic$end(float end) {
		return end * factor();
	}
	//#endif

	private static float factor() {
		ClientConfig c = ArcticClient.config();
		return c == null || !c.fog ? 1f : c.fogOff ? 1000f : (float) Math.max(1.0, Math.min(10.0, c.fogScale));
	}
}
//#endif
