package com.arcticlauncher.polonium.mixin;

import net.minecraft.client.gui.Font;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(targets = "net.minecraft.client.renderer.feature.NameTagFeatureRenderer$GlyphRenderer")
public interface GlyphRendererAccess {
	@Accessor("displayMode")
	Font.DisplayMode polonium$displayMode();

	@Accessor("pose")
	org.joml.Matrix4f polonium$pose();

	@Accessor("lightCoords")
	int polonium$lightCoords();
}
