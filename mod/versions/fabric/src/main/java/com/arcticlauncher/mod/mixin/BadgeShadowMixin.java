package com.arcticlauncher.mod.mixin;

//#if MC >= 1.16 && MC < 1.21.4
import com.arcticlauncher.mod.BadgeTexture;
import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.client.gui.font.glyphs.EmptyGlyph;
import net.minecraft.network.chat.Style;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;

/** Keep the badge's advance in the shadow pass while omitting its quad. The name text still has a shadow. */
@Mixin(targets = "net.minecraft.client.gui.Font$StringRenderOutput")
abstract class BadgeShadowMixin {
	@Shadow @Final private boolean dropShadow;
	@Unique private static final BakedGlyph ARCTIC_EMPTY = new EmptyGlyph();

	@ModifyExpressionValue(
			method = "accept(ILnet/minecraft/network/chat/Style;I)Z",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/font/FontSet;getGlyph(I)Lnet/minecraft/client/gui/font/glyphs/BakedGlyph;"))
	private BakedGlyph arctic$omitShadow(BakedGlyph quad, int index, Style style, int codepoint) {
		if (dropShadow) {
			int slot = BadgeTexture.slot(codepoint, style.getFont());
			if (slot >= 0) return ARCTIC_EMPTY;
		}
		return quad;
	}
}
//#endif
