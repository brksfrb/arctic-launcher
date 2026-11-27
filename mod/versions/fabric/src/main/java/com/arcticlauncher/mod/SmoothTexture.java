package com.arcticlauncher.mod;

import com.mojang.blaze3d.platform.NativeImage;
//#if MC >= 1.21.11
import com.mojang.blaze3d.systems.RenderSystem;
//#if MC >= 26.3
import com.mojang.renderpearl.api.textures.FilterMode;
//#else
import com.mojang.blaze3d.textures.FilterMode;
//#endif
//#endif
import net.minecraft.client.renderer.texture.DynamicTexture;

/** A dynamic texture that keeps linear filtering when the render state binds it. */
final class SmoothTexture extends DynamicTexture {
	SmoothTexture(String name, NativeImage image) {
		//#if MC >= 1.21.5
		super(() -> name, image);
		//#else
		super(image);
		//#endif
		//#if MC >= 1.21.11
		this.sampler = RenderSystem.getSamplerCache().getClampToEdge(FilterMode.LINEAR);
		//#else
		setFilter(true, false);
		//#endif
	}

	//#if MC < 1.21.11
	@Override
	public void setFilter(boolean blur, boolean mipmap) {
		// Older text render types apply their default filter on every bind.
		super.setFilter(true, false);
	}
	//#endif
}
