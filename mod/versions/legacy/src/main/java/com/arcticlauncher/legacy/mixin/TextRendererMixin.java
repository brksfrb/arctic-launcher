package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.feature.Streamer;
import net.minecraft.client.font.TextRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Streamer mode: your name and the server address are hidden in every piece of text. And the Arctic
 * badge: text that starts with its blank mark (player list, name tags) gets the snowflake picture in
 * that place.
 */
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

	/** The badge's mark is as wide as the picture and its gap (Badge.ADVANCE) in every font, Fancy on or off. */
	@Inject(method = "getStringWidth", at = @At("HEAD"), cancellable = true)
	private void arctic$badgeWidth(String text, CallbackInfoReturnable<Integer> cir) {
		if (text == null || !text.startsWith(com.arcticlauncher.client.feature.Badge.MARK)) {
			return;
		}
		String rest = text;
		while (rest.startsWith(com.arcticlauncher.client.feature.Badge.MARK)) {
			rest = rest.substring(com.arcticlauncher.client.feature.Badge.MARK.length());
		}
		// Only the first mark has a picture; the others are dropped when drawn.
		cir.setReturnValue(com.arcticlauncher.client.feature.Badge.ADVANCE + ((TextRenderer) (Object) this).getStringWidth(rest));
	}

	@Inject(method = "draw(Ljava/lang/String;FFIZ)I", at = @At("HEAD"), cancellable = true)
	private void arctic$badge(String text, float x, float y, int color, boolean shadow, CallbackInfoReturnable<Integer> cir) {
		if (text == null || !text.startsWith(com.arcticlauncher.client.feature.Badge.MARK)) {
			return;
		}
		TextRenderer self = (TextRenderer) (Object) this;
		int width = com.arcticlauncher.client.feature.Badge.ADVANCE;
		com.arcticlauncher.legacy.LegacyHooks.drawBadge(x, y, color, shadow);
		// Every leading mark goes (text from a server can repeat it), so the redraw below never meets one again.
		String rest = text;
		while (rest.startsWith(com.arcticlauncher.client.feature.Badge.MARK)) {
			rest = rest.substring(com.arcticlauncher.client.feature.Badge.MARK.length());
		}
		cir.setReturnValue(self.draw(rest, x + width, y, color, shadow));
	}
}
