package com.arcticlauncher.mod;
//#if MC >= 1.19.3
import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.resources.Identifier;
public interface GlassSpriteSource {
	NativeImage arctic$image();
	SpriteContents arctic$variant(Identifier id, NativeImage image);
}
//#endif
