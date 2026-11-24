//#if MC >= 1.13
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.client.replay.Replays.ViewerAction;
import com.mojang.blaze3d.platform.InputConstants;
import java.util.Arrays;
import java.util.Map;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Options;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Options.class)
abstract class ReplayKeyOptionsMixin {
	//#if MC >= 1.14
	@Mutable @Final
	//#endif
	@Shadow public KeyMapping[] keyMappings;
	@Shadow public abstract void save();

	@Unique private boolean arctic$replayKeysAdded;
	//#if MC >= 1.21.9
	@Unique private static KeyMapping.Category arctic$replayCategory;
	//#endif

	@Inject(method = "load", at = @At("HEAD"))
	private void arctic$registerReplayKeys(CallbackInfo ci) {
		if (arctic$replayKeysAdded) return;
		arctic$replayKeysAdded = true;
		//#if MC >= 1.21.9
		if (arctic$replayCategory == null) {
			//#if MC >= 1.21.11
			arctic$replayCategory = KeyMapping.Category.register(net.minecraft.resources.Identifier.fromNamespaceAndPath("arctic", "replay"));
			//#else
			arctic$replayCategory = KeyMapping.Category.register(net.minecraft.resources.ResourceLocation.fromNamespaceAndPath("arctic", "replay"));
			//#endif
		}
		KeyMapping.Category category = arctic$replayCategory;
		//#else
		String category = "key.categories.arctic.replay";
		//#endif
		Map<String, String> previous = Replays.previousViewerKeys(FabricLoader.getInstance().getConfigDir().toFile());
		ViewerAction[] actions = ViewerAction.values();
		int index = keyMappings.length;
		keyMappings = Arrays.copyOf(keyMappings, index + actions.length);
		for (ViewerAction action : actions) {
			KeyMapping mapping = new KeyMapping(action.translationKey, InputConstants.getKey(action.defaultKey()).getValue(), category);
			String old = previous.get(action.name());
			if (old != null) {
				try { mapping.setKey(InputConstants.getKey(old)); } catch (IllegalArgumentException ignored) { /* Keep the default. */ }
			}
			keyMappings[index++] = mapping;
			action.register(mapping::saveString, key -> {
				mapping.setKey(InputConstants.getKey(key));
				KeyMapping.resetMapping();
				save();
			});
		}
	}
}
//#endif
