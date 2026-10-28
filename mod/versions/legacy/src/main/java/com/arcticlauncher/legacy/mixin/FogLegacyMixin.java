package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.render.GameRenderer;
import org.lwjgl.opengl.GL11;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Fog: the distance where the fog starts and ends is stretched by the player's factor (or pushed out of sight). */
@Mixin(GameRenderer.class)
abstract class FogLegacyMixin {
	@Inject(method = "renderFog", at = @At("TAIL"))
	private void arctic$fog(int mode, float tickDelta, CallbackInfo ci) {
		ClientConfig c = ArcticClient.config();
		if (c == null || !c.fog) {
			return;
		}
		float f = c.fogOff ? 1000f : (float) Math.max(1.0, Math.min(10.0, c.fogScale));
		GL11.glFogf(GL11.GL_FOG_START, GL11.glGetFloat(GL11.GL_FOG_START) * f);
		GL11.glFogf(GL11.GL_FOG_END, GL11.glGetFloat(GL11.GL_FOG_END) * f);
	}
}
