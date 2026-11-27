package com.arcticlauncher.mod.mixin;

//#if MC >= 1.16 && MC < 1.21.9
import com.arcticlauncher.mod.ChatStampFont;
import com.mojang.blaze3d.font.GlyphInfo;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(FontSet.class)
abstract class ChatStampFontSetMixin {
	@Shadow @Final private Identifier name;

	@Inject(method = "getGlyphInfo", at = @At("HEAD"), cancellable = true)
	private void arctic$advance(int codepoint,
			//#if MC >= 1.19
			boolean filterFishy,
			//#endif
			CallbackInfoReturnable<GlyphInfo> cir) {
		if (ChatStampFont.ID.equals(name)) cir.setReturnValue(ChatStampFont.info(codepoint));
	}

	@Inject(method = "getGlyph", at = @At("HEAD"), cancellable = true)
	private void arctic$blank(int codepoint, CallbackInfoReturnable<BakedGlyph> cir) {
		if (ChatStampFont.ID.equals(name)) cir.setReturnValue(ChatStampFont.BLANK);
	}
}
//#endif
