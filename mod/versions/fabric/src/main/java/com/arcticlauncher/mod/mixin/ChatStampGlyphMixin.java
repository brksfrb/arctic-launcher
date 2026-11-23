package com.arcticlauncher.mod.mixin;

//#if MC >= 1.16
import com.arcticlauncher.mod.ChatStampFont;
import net.minecraft.client.gui.Font;
//#if MC >= 1.21.9
import net.minecraft.client.gui.GlyphSource;
import net.minecraft.network.chat.FontDescription;
//#else
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Unique;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(Font.class)
abstract class ChatStampGlyphMixin {
	//#if MC >= 1.21.9
	@Inject(method = "getGlyphSource", at = @At("HEAD"), cancellable = true)
	private void arctic$spacers(FontDescription font, CallbackInfoReturnable<GlyphSource> cir) {
		if (ChatStampFont.ID.equals(font)) cir.setReturnValue(ChatStampFont.INSTANCE);
	}
	//#else
	@Unique private FontSet arctic$stampFont;

	@Inject(method = "getFontSet", at = @At("HEAD"), cancellable = true)
	private void arctic$spacers(Identifier font, CallbackInfoReturnable<FontSet> cir) {
		if (!ChatStampFont.ID.equals(font)) return;
		if (arctic$stampFont == null) {
			// This set provides only empty glyphs, so it needs no atlas or resource reload.
			arctic$stampFont = new FontSet(Minecraft.getInstance().getTextureManager(), ChatStampFont.ID);
		}
		cir.setReturnValue(arctic$stampFont);
	}
	//#endif
}
//#endif
