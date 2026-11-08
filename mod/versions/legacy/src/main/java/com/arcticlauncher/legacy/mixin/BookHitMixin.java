package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacySmoothFont;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.screen.ingame.BookEditScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * A book's lines are drawn {@code fontHeight} apart and clicks divide by the same field. With the
 * smooth font that field stays 9 while the letters are taller, so a click hits the neighbouring
 * line. Drawing and {@code getTextAt} both read the font's own line height.
 */
@Mixin(BookEditScreen.class)
abstract class BookHitMixin {
	@Redirect(method = {"render", "getTextAt"}, at = @At(value = "FIELD", target = "Lnet/minecraft/client/font/TextRenderer;fontHeight:I"))
	private int arctic$height(TextRenderer font) {
		return LegacySmoothFont.aligned() ? LegacySmoothFont.lineHeight() : font.fontHeight;
	}
}
