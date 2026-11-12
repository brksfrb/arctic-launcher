package com.arcticlauncher.mod.mixin;
//#if MC >= 1.15 && MC < 1.19.3
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.metadata.animation.AnimationMetadataSection;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;
@Mixin(TextureAtlasSprite.Info.class)
public interface GlassInfoAccess {
	@Accessor("metadata") AnimationMetadataSection arctic$metadata();
}
//#endif
