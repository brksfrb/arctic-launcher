package com.arcticlauncher.mod.mixin;

//#if MC >= 1.14
import com.arcticlauncher.client.ArcticClient;
//#if MC >= 26.1
import net.minecraft.client.renderer.LightmapRenderStateExtractor;
import net.minecraft.client.renderer.state.LightmapRenderState;
//#else
import net.minecraft.client.renderer.LightTexture;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Fullbright takes effect when it's switched, not whenever the game next redraws its light map. Mods
 * that update the light map only when something they watch changed (BadOptimizations' "lightmap
 * caching" watches the sky, the water, lightning, the End flash... but not Fullbright) would otherwise
 * keep the old brightness until a day passes: the switch seemed to do nothing. Switching marks the
 * light map as needing an update.
 */
//#if MC >= 26.1
@Mixin(LightmapRenderStateExtractor.class)
//#else
@Mixin(LightTexture.class)
//#endif
abstract class LightmapRefreshMixin {
	//#if MC >= 26.1
	@Shadow
	private boolean needsUpdate;
	//#else
	@Shadow
	private boolean updateLightTexture;
	//#endif

	private boolean arctic$lastFullbright;

	//#if MC >= 26.1
	@Inject(method = "extract", at = @At("HEAD"), require = 0)
	private void arctic$refresh(LightmapRenderState state, float partialTicks, CallbackInfo ci) {
	//#else
	@Inject(method = "updateLightTexture", at = @At("HEAD"), require = 0)
	private void arctic$refresh(float partialTicks, CallbackInfo ci) {
	//#endif
		boolean on = ArcticClient.features() != null && ArcticClient.features().fullbright();
		if (on != arctic$lastFullbright) {
			arctic$lastFullbright = on;
			//#if MC >= 26.1
			needsUpdate = true;
			//#else
			updateLightTexture = true;
			//#endif
		}
	}
}
//#endif
