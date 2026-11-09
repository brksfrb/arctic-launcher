package com.arcticlauncher.mod.mixin;
//#if MC < 1.21.5
import net.minecraft.client.renderer.block.model.BakedQuad;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;
@Mixin(BakedQuad.class)
public interface GlassQuadAccess {
	@Accessor("sprite") TextureAtlasSprite arctic$sprite();
}
//#endif
