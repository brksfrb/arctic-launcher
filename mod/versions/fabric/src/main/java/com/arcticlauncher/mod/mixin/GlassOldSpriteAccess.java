package com.arcticlauncher.mod.mixin;
//#if MC >= 1.14 && MC < 1.15
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;
@Mixin(TextureAtlasSprite.class)
public interface GlassOldSpriteAccess {
	@Invoker("<init>") static TextureAtlasSprite arctic$new(Identifier id, int width, int height) { throw new AssertionError(); }
}
//#endif
