package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.MenuKeyMapping;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Options;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Adds the Arctic menu key to Minecraft's key bindings before options.txt is read: Controls lists it and the file keeps it. */
@Mixin(Options.class)
abstract class OptionsKeyMixin {
	@Shadow
	@Mutable
	@Final
	public KeyMapping[] keyMappings;

	@Inject(method = "load", at = @At("HEAD"))
	private void arctic$menuKey(CallbackInfo ci) {
		keyMappings = MenuKeyMapping.with(keyMappings);
	}
}
