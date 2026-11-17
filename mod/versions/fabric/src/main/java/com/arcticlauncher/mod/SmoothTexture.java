package com.arcticlauncher.mod;

//#if MC >= 1.21.11
import com.mojang.blaze3d.platform.NativeImage;
import com.mojang.blaze3d.systems.RenderSystem;
//#if MC >= 26.3
import com.mojang.renderpearl.api.textures.FilterMode;
//#else
import com.mojang.blaze3d.textures.FilterMode;
//#endif
import net.minecraft.client.renderer.texture.DynamicTexture;

/**
 * A picture that filters linearly (the game's own dynamic textures are nearest), for the
 * title's spinning snowflake. From 1.21.11 the filter is a sampler the texture sets in its
 * constructor, which only a subclass can replace.
 */
final class SmoothTexture extends DynamicTexture {
	SmoothTexture(String name, NativeImage image) {
		super(() -> name, image);
		this.sampler = RenderSystem.getSamplerCache().getClampToEdge(FilterMode.LINEAR);
	}
}
//#endif
