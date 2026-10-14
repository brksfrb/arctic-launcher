package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.replay.SoundCapture;
import java.util.Map;
import net.minecraft.client.option.GameOptions;
import net.minecraft.client.sound.SoundCategory;
import net.minecraft.client.sound.SoundInstance;
import net.minecraft.client.sound.SoundSystem;
//#if MC < 1.12
import net.minecraft.client.sound.Sound;
//#endif
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(SoundSystem.class)
abstract class ReplaySoundMixin {
	@Shadow @Final private Map<SoundInstance, String> field_8196;
	@Shadow @Final private GameOptions options;
	//#if MC >= 1.12
	@Shadow protected abstract float method_12539(SoundInstance s);
	@Shadow protected abstract float method_12540(SoundInstance s);
	//#else
	@Shadow private Map<SoundInstance, Sound> sources;
	@Shadow @Final private net.minecraft.client.sound.SoundManager manager;
	@Shadow protected abstract float method_7099(SoundInstance s, Sound sound);
	@Shadow protected abstract float method_7100(SoundInstance s, Sound sound, SoundCategory category);
	//#endif
	@Unique private final SoundCapture arctic$audio = new SoundCapture();
	@Inject(method = "tick", at = @At("TAIL"))
	private void arctic$capture(CallbackInfo ci) {
		if (!arctic$audio.begin()) return;
		for (SoundInstance s : field_8196.keySet()) {
			//#if MC >= 1.12
			if (s.method_12533() == null) continue;
			String path = s.method_12533().method_12523().toString();
			float volume = method_12540(s), pitch = method_12539(s);
			//#else
			Sound sound = sources.get(s);
			if (sound == null) continue;
			String path = sound.getIdentifier().toString();
			float volume = method_7100(s, sound, manager.get(s.getIdentifier()).getSoundCategory()), pitch = method_7099(s, sound);
			//#endif
			volume *= options.getSoundVolume(SoundCategory.MASTER);
			arctic$audio.sound(s, path, volume, pitch, s.isRepeatable() && s.getRepeatDelay() == 0,
					s.getX(), s.getY(), s.getZ(), false,
					s.getAttenuationType() == SoundInstance.AttenuationType.NONE ? 0 : Math.max(1, s.getVolume()) * 16);
		}
		arctic$audio.end();
	}
}
