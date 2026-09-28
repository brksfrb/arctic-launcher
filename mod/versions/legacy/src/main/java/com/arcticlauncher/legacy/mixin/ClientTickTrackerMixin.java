package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.replay.ReplayClock;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.ClientTickTracker;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * While a replay is open the game ticks by its clock (paused, faster,
 * slower, or exact steps while exporting); otherwise Minecraft's own timing.
 */
@Mixin(ClientTickTracker.class)
abstract class ClientTickTrackerMixin {
	@Unique
	private static final float MS_PER_TICK = 50f;

	@Shadow
	public int ticksThisFrame;
	@Shadow
	public float tickDelta;
	@Shadow
	public float lastFrameDuration;

	@Unique
	private long arctic$last = -1;

	@Inject(method = "tick", at = @At("HEAD"), cancellable = true)
	private void arctic$replayClock(CallbackInfo ci) {
		long now = ReplayClock.gameMillis(MinecraftClient.getTime());
		long last = arctic$last;
		arctic$last = now;
		if (!ReplayClock.active() || last < 0) {
			return;
		}
		lastFrameDuration = Math.max(0, now - last) / MS_PER_TICK;
		tickDelta += lastFrameDuration;
		ticksThisFrame = (int) tickDelta;
		tickDelta -= ticksThisFrame;
		ci.cancel();
	}
}
