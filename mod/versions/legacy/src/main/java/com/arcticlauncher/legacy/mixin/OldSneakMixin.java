//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.render.GameRenderer;
import net.minecraft.entity.Entity;
import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * 1.7 sneaking, 1.8.9: the camera sinks and rises smoothly when you sneak (1.7 let its camera offset
 * fade by 0.4 every tick; 1.8 moves it at once).
 */
@Mixin(GameRenderer.class)
abstract class OldSneakMixin {
	/** What is left of the offset after one tick (50 ms), as 1.7 kept it. */
	private static final double KEPT_PER_TICK = 0.4;
	private static final double TICK_SECONDS = 0.05;
	/** A longer gap than this (the game was paused, another view) starts over at the true height. */
	private static final long RESET_NANOS = 500_000_000L;

	@Unique
	private static float arctic$eye = Float.NaN;
	@Unique
	private static long arctic$last;
	@Unique
	private static Entity arctic$entity;

	@Redirect(method = "transformCamera", at = @At(value = "INVOKE", target = "Lnet/minecraft/entity/Entity;getEyeHeight()F", ordinal = 0), require = 0)
	private float arctic$eyeHeight(Entity entity) {
		float target = entity.getEyeHeight();
		if (!ArcticClient.config().oldSneak || !(entity instanceof PlayerEntity)) {
			arctic$eye = Float.NaN;
			return target;
		}
		long now = System.nanoTime();
		if (Float.isNaN(arctic$eye) || entity != arctic$entity || now - arctic$last > RESET_NANOS) {
			arctic$eye = target;
		} else {
			double seconds = (now - arctic$last) / 1e9;
			arctic$eye += (target - arctic$eye) * (1.0 - Math.pow(KEPT_PER_TICK, seconds / TICK_SECONDS));
		}
		arctic$entity = entity;
		arctic$last = now;
		return arctic$eye;
	}
}
//#endif
