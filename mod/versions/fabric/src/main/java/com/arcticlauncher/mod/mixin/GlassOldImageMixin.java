package com.arcticlauncher.mod.mixin;
//#if MC < 1.19.3
import com.arcticlauncher.client.ctm.GlassTiles;
import com.arcticlauncher.mod.GlassImages;
import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
//#if MC >= 1.15
import com.llamalad7.mixinextras.sugar.Local;
//#else
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.injection.Redirect;
import java.io.InputStream;
import java.io.IOException;
import net.minecraft.resources.Identifier;
//#endif
@Mixin(TextureAtlasSprite.class)
abstract class GlassOldImageMixin {
	//#if MC >= 1.15
	@ModifyVariable(method = "<init>", at = @At("HEAD"), argsOnly = true)
	private NativeImage arctic$image(NativeImage image, @Local(argsOnly = true) TextureAtlasSprite.Info info) {
		if (!info.name().toString().startsWith("arctic:ctm/")) return image;
		NativeImage generated = GlassImages.generate(image, info.width(), info.height(), GlassTiles.tile(info.name().toString()), GlassTiles.original(info.name().toString()));
		image.close();
		return generated;
	}
	//#else
	@Shadow @Final private Identifier name;
	@Shadow @Final protected int width;
	@Shadow @Final protected int height;
	@Redirect(method = "loadData", at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/platform/NativeImage;read(Ljava/io/InputStream;)Lcom/mojang/blaze3d/platform/NativeImage;"))
	private NativeImage arctic$image(InputStream stream) throws IOException {
		NativeImage image = NativeImage.read(stream);
		if (!name.toString().startsWith("arctic:ctm/")) return image;
		NativeImage generated = GlassImages.generate(image, width, height, GlassTiles.tile(name.toString()), GlassTiles.original(name.toString()));
		image.close();
		return generated;
	}
	//#endif
}
//#endif
