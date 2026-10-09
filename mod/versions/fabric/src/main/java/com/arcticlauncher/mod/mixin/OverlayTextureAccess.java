package com.arcticlauncher.mod.mixin;

import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.client.renderer.texture.OverlayTexture;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The hurt-tint texture, recolored for Hit color. */
@Mixin(OverlayTexture.class)
public interface OverlayTextureAccess {
	@Accessor("texture")
	DynamicTexture arctic$texture();
}
