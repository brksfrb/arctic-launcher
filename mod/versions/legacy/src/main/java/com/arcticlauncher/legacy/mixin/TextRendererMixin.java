package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.feature.Streamer;
import net.minecraft.client.font.TextRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** Streamer mode: your name and the server address are hidden in every piece of text. */
@Mixin(TextRenderer.class)
abstract class TextRendererMixin {
	@ModifyVariable(method = "draw(Ljava/lang/String;FFIZ)I", at = @At("HEAD"), argsOnly = true)
	private String arctic$maskDraw(String text) {
		return Streamer.mask(text);
	}

	@ModifyVariable(method = "getStringWidth", at = @At("HEAD"), argsOnly = true)
	private String arctic$maskWidth(String text) {
		return Streamer.mask(text);
	}
}
