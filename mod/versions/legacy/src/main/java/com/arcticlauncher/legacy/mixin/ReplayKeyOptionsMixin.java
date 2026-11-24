package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.client.replay.Replays.ViewerAction;
import com.arcticlauncher.legacy.LegacyKeys;
import java.util.Arrays;
import java.io.File;
import java.util.Map;
import net.minecraft.client.option.GameOptions;
import net.minecraft.client.option.KeyBinding;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(GameOptions.class)
abstract class ReplayKeyOptionsMixin {
	@Shadow public KeyBinding[] allKeys;
	@Shadow private File optionsFile;
	@Shadow public abstract void save();
	@Unique private boolean arctic$replayKeysAdded;

	@Inject(method = "load", at = @At("HEAD"))
	private void arctic$registerReplayKeys(CallbackInfo ci) {
		if (arctic$replayKeysAdded) return;
		arctic$replayKeysAdded = true;
		Map<String, String> previous = Replays.previousViewerKeys(new File(optionsFile.getParentFile(), "config"));
		ViewerAction[] actions = ViewerAction.values();
		int index = allKeys.length;
		allKeys = Arrays.copyOf(allKeys, index + actions.length);
		for (ViewerAction action : actions) {
			KeyBinding binding = new KeyBinding(action.translationKey, LegacyKeys.code(action.defaultKey()), "key.categories.arctic.replay");
			binding.setCode(LegacyKeys.code(previous.getOrDefault(action.name(), action.defaultKey())));
			allKeys[index++] = binding;
			action.register(() -> LegacyKeys.name(binding.getCode()), key -> {
				binding.setCode(LegacyKeys.code(key));
				KeyBinding.updateKeysByCode();
				save();
			});
		}
	}
}
