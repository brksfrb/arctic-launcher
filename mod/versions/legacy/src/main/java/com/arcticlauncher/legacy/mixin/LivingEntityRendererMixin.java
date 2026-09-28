package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.render.entity.LivingEntityRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/** Hit color: the tint on hurt mobs and players (red, green, blue of the overlay). */
@Mixin(LivingEntityRenderer.class)
abstract class LivingEntityRendererMixin {
	private static final String PUT = "Ljava/nio/FloatBuffer;put(F)Ljava/nio/FloatBuffer;";

	@ModifyArg(method = "method_10252", at = @At(value = "INVOKE", target = PUT, ordinal = 0))
	private float arctic$red(float value) {
		return channel(value, 16);
	}

	@ModifyArg(method = "method_10252", at = @At(value = "INVOKE", target = PUT, ordinal = 1))
	private float arctic$green(float value) {
		return channel(value, 8);
	}

	@ModifyArg(method = "method_10252", at = @At(value = "INVOKE", target = PUT, ordinal = 2))
	private float arctic$blue(float value) {
		return channel(value, 0);
	}

	private static float channel(float value, int shift) {
		int color = ArcticClient.config().hitColor;
		return color == 0 ? value : (color >> shift & 0xFF) / 255f;
	}
}
