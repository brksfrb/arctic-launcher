package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.TerrainWork;
import net.minecraft.client.world.BuiltChunk;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** How long this section's last build took, on whichever thread built it. */
@Mixin(BuiltChunk.class)
abstract class TerrainBuildTimeMixin implements TerrainWork.Timed {
	@Unique
	private volatile long arctic$buildStart;
	@Unique
	private volatile long arctic$buildNanos;

	@Inject(method = "method_10164", at = @At("HEAD"), require = 0)
	private void arctic$buildStart(CallbackInfo ci) {
		arctic$buildStart = System.nanoTime();
	}

	@Inject(method = "method_10164", at = @At("RETURN"), require = 0)
	private void arctic$buildEnd(CallbackInfo ci) {
		arctic$buildNanos = System.nanoTime() - arctic$buildStart;
	}

	@Override
	public long arctic$buildNanos() {
		return arctic$buildNanos;
	}
}
