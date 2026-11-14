package com.arcticlauncher.legacy.mixin;
import com.arcticlauncher.client.ctm.GlassTiles;
import java.awt.image.BufferedImage;
import net.minecraft.client.texture.Sprite;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.Redirect;
@Mixin(Sprite.class)
abstract class GlassLegacyImageMixin {
	@Shadow @Final private String name;
	//#if MC < 1.9
	@ModifyVariable(method = "method_7009", at = @At("HEAD"), argsOnly = true)
	private BufferedImage[] arctic$images(BufferedImage[] images) {
		if (!name.startsWith("arctic:ctm/")) return images;
		// Mixin 0.7 resolves invocation owners as classes; an array clone owner is not loadable.
		BufferedImage[] result = java.util.Arrays.copyOf(images, images.length);
		for (int i = 0; i < result.length; i++) if (result[i] != null) result[i] = arctic$generate(result[i]);
		return result;
	}
	//#else
	@Redirect(method = "method_12492", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/texture/TextureUtil;create(Ljava/io/InputStream;)Ljava/awt/image/BufferedImage;"))
	private BufferedImage arctic$image(java.io.InputStream stream) throws java.io.IOException {
		BufferedImage image = net.minecraft.client.texture.TextureUtil.create(stream);
		return name.startsWith("arctic:ctm/") ? arctic$generate(image) : image;
	}
	//#endif
	private BufferedImage arctic$generate(BufferedImage image) {
		int w = image.getWidth(), h = image.getHeight();
		int[] pixels = image.getRGB(0, 0, w, h, null, 0, w);
		BufferedImage preferred = com.arcticlauncher.legacy.LegacyGlassCtm.packs.tile(GlassTiles.original(name), GlassTiles.tile(name));
		if (preferred == null) {
			if (GlassTiles.clear(GlassTiles.original(name))) pixels = GlassTiles.clarity(pixels, w, h, w, w, com.arcticlauncher.client.ctm.GlassLines.stitched());
			pixels = GlassTiles.generate(pixels, w, h, w, w, GlassTiles.tile(name));
		}
		else for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) pixels[y * w + x] = preferred.getRGB(x * preferred.getWidth() / w, (y % w) * preferred.getHeight() / w);
		BufferedImage result = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
		result.setRGB(0, 0, w, h, pixels, 0, w);
		return result;
	}
}
