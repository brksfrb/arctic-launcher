package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.feature.Streamer;
import com.arcticlauncher.client.feature.Badge;
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

	@Inject(method = "getStringWidth", at = @At("HEAD"), cancellable = true)
	private void arctic$badgeWidth(String text, CallbackInfoReturnable<Integer> cir) {
		if (!Badge.marked(text)) return;
		cir.setReturnValue(Badge.advance(Badge.markedTab(text)) + ((TextRenderer) (Object) this).getStringWidth(Badge.unmark(text)));
	}

	@Inject(method = "draw(Ljava/lang/String;FFIZ)I", at = @At("HEAD"), cancellable = true)
	private void arctic$badge(String text, float x, float y, int color, boolean shadow, CallbackInfoReturnable<Integer> cir) {
		if (!Badge.marked(text)) return;
		boolean tab = Badge.markedTab(text);
		com.arcticlauncher.legacy.LegacyHooks.drawBadge(x, y, color, shadow, Badge.markedStyle(text), tab);
		cir.setReturnValue(((TextRenderer) (Object) this).draw(Badge.unmark(text), x + Badge.advance(tab), y, color, shadow));
	}
}
