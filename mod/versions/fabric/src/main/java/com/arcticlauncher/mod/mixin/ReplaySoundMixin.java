package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.SoundCapture;
import java.util.Map;
import net.minecraft.client.Options;
import net.minecraft.client.resources.sounds.SoundInstance;
import net.minecraft.client.sounds.SoundEngine;
import net.minecraft.sounds.SoundSource;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Observe resolved, active channels: no changes to the game's sound playback. */
@Mixin(SoundEngine.class)
abstract class ReplaySoundMixin {
	@Shadow @Final private Map<SoundInstance, ?> instanceToChannel;
	@Shadow @Final private Options options;
	@Shadow protected abstract float calculateVolume(SoundInstance sound);
	@Shadow protected abstract float calculatePitch(SoundInstance sound);
	@Unique private final SoundCapture arctic$audio = new SoundCapture();
	@Inject(method = "tick", at = @At("TAIL"))
	private void arctic$capture(boolean paused, CallbackInfo ci) {
		if (!arctic$audio.begin()) return;
		for (SoundInstance s : instanceToChannel.keySet()) {
			if (s.getSound() == null) continue;
			float volume = calculateVolume(s) * options.getSoundSourceVolume(SoundSource.MASTER);
			arctic$audio.sound(s, s.getSound().getPath().toString(), volume, calculatePitch(s),
					s.isLooping() && s.getDelay() == 0, s.getX(), s.getY(), s.getZ(), s.isRelative(),
					s.getAttenuation() == SoundInstance.Attenuation.NONE ? 0 : Math.max(1, s.getVolume()) * s.getSound().getAttenuationDistance());
		}
		arctic$audio.end();
	}
}
