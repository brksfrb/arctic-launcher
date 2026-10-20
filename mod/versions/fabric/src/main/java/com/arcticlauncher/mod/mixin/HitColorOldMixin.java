package com.arcticlauncher.mod.mixin;

//#if MC < 1.15
import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/** Hit color on 1.14: the tint on hurt mobs and players is put together for each one (red, green, blue of the overlay). */
@Mixin(LivingEntityRenderer.class)
abstract class HitColorOldMixin {
	private static final String PUT = "Ljava/nio/FloatBuffer;put(F)Ljava/nio/FloatBuffer;";

	@ModifyArg(method = "setupOverlayColor(Lnet/minecraft/world/entity/LivingEntity;FZ)Z", at = @At(value = "INVOKE", target = PUT, ordinal = 0))
	private float arctic$red(float value) {
		return channel(value, 16);
	}

	@ModifyArg(method = "setupOverlayColor(Lnet/minecraft/world/entity/LivingEntity;FZ)Z", at = @At(value = "INVOKE", target = PUT, ordinal = 1))
	private float arctic$green(float value) {
		return channel(value, 8);
	}

	@ModifyArg(method = "setupOverlayColor(Lnet/minecraft/world/entity/LivingEntity;FZ)Z", at = @At(value = "INVOKE", target = PUT, ordinal = 2))
	private float arctic$blue(float value) {
		return channel(value, 0);
	}

	private static float channel(float value, int shift) {
		int color = ArcticClient.config().hitColor;
		return color == 0 ? value : (color >> shift & 0xFF) / 255f;
	}
}
//#endif
