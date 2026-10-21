package com.arcticlauncher.legacy.mixin;

import net.minecraft.client.MinecraftClient;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** The game's own "the window changed size" step, which is private. */
@Mixin(MinecraftClient.class)
public interface MinecraftClientResizeInvoker {
	@Invoker("onResolutionChanged")
	void arctic$resize(int width, int height);
}
