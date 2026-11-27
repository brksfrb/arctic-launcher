package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyKeys;
import net.minecraft.client.option.GameOptions;
import net.minecraft.client.option.KeyBinding;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Puts the Arctic menu key among Minecraft's key bindings before the options
 * file is read, so Controls lists it (under "Arctic") and options.txt keeps it.
 */
@Mixin(GameOptions.class)
abstract class GameOptionsMixin {
	@Shadow
	@Mutable
	@Final
	public KeyBinding[] allKeys;

	@Inject(method = "load", at = @At("HEAD"))
	private void arctic$menuKey(CallbackInfo ci) {
		KeyBinding binding = LegacyKeys.menuBinding();
		for (KeyBinding k : allKeys) {
			if (k == binding) {
				return;
			}
		}
		KeyBinding[] all = java.util.Arrays.copyOf(allKeys, allKeys.length + 1);
		all[allKeys.length] = binding;
		allKeys = all;
	}
}
