package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.9
import com.arcticlauncher.mod.ChatStampFont;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GlyphSource;
import net.minecraft.network.chat.FontDescription;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(Font.class)
abstract class ChatStampGlyphMixin {
	@Inject(method = "getGlyphSource", at = @At("HEAD"), cancellable = true)
	private void arctic$spacers(FontDescription font, CallbackInfoReturnable<GlyphSource> cir) {
		if (ChatStampFont.ID.equals(font)) cir.setReturnValue(ChatStampFont.INSTANCE);
	}
}
//#endif
