package com.arcticlauncher.mod.mixin;

//#if MC >= 1.14 && MC < 1.20.5
import com.arcticlauncher.client.ArcticClient;
import com.mojang.blaze3d.platform.NativeImage;
import org.lwjgl.stb.STBTTFontinfo;
import org.lwjgl.system.MemoryUtil;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Pixel-sharp smooth font: a glyph image the game rasterized (one byte of coverage per pixel) gets every
 * pixel fully on or off, unless anti-aliasing is asked for in the menu style.
 */
@Mixin(NativeImage.class)
abstract class FontSharpMixin {
	/** Below this much coverage a pixel is off. */
	private static final int SHARP_AT = 96;

	@Shadow
	private long pixels;
	@Shadow
	@org.spongepowered.asm.mixin.Final
	private int width;
	@Shadow
	@org.spongepowered.asm.mixin.Final
	private int height;

	@Inject(method = "copyFromFont", at = @At("RETURN"), require = 0)
	private void arctic$sharp(STBTTFontinfo font, int glyph, int w, int h, float sx, float sy, float shiftX, float shiftY,
			int x0, int y0, CallbackInfo ci) {
		if (ArcticClient.config() == null || ArcticClient.config().fontAntialias || pixels == 0) {
			return;
		}
		int count = width * height;
		for (int i = 0; i < count; i++) {
			MemoryUtil.memPutByte(pixels + i, (byte) ((MemoryUtil.memGetByte(pixels + i) & 0xFF) >= SHARP_AT ? 0xFF : 0));
		}
	}
}
//#endif
