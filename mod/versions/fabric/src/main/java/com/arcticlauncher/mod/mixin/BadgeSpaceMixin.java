package com.arcticlauncher.mod.mixin;
//#if MC < 1.19
import com.arcticlauncher.client.feature.Badge;
import com.mojang.blaze3d.font.GlyphInfo;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.client.gui.font.glyphs.EmptyGlyph;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Equivalent of our space providers before the game had that provider type. */
@Mixin(FontSet.class)
abstract class BadgeSpaceMixin {
	@Shadow @Final private Identifier name;
	private static final GlyphInfo ARCTIC_GAP = () -> 1;
	private static final GlyphInfo ARCTIC_TAB_GAP = () -> Badge.advance(true);
	private static final GlyphInfo ARCTIC_TAG_GAP = () -> Badge.advance(false);
	private static final BakedGlyph ARCTIC_EMPTY = new EmptyGlyph();
	private boolean arctic$space(int c) {
		return c == 0xe0af || c == 0xe0c0 || c == 0xe0c1 || c == ' ' && name.getNamespace().equals("arctic") && name.getPath().startsWith("badge");
	}
	@Inject(method = "getGlyphInfo", at = @At("HEAD"), cancellable = true)
	//#if MC < 1.16
	private void arctic$info(char c, CallbackInfoReturnable<GlyphInfo> cir) {
	//#else
	private void arctic$info(int c, CallbackInfoReturnable<GlyphInfo> cir) {
	//#endif
		if (arctic$space(c)) cir.setReturnValue(c == 0xe0c0 ? ARCTIC_TAB_GAP : c == 0xe0c1 ? ARCTIC_TAG_GAP : ARCTIC_GAP);
	}
	@Inject(method = "getGlyph", at = @At("HEAD"), cancellable = true)
	//#if MC < 1.16
	private void arctic$glyph(char c, CallbackInfoReturnable<BakedGlyph> cir) {
	//#else
	private void arctic$glyph(int c, CallbackInfoReturnable<BakedGlyph> cir) {
	//#endif
		if (arctic$space(c)) cir.setReturnValue(ARCTIC_EMPTY);
	}
}
//#endif
