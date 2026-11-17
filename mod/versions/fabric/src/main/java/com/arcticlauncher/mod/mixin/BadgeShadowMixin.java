package com.arcticlauncher.mod.mixin;

//#if MC >= 1.16 && MC < 1.21.4
import net.minecraft.network.chat.Style;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The player list draws names with a shadow; the Arctic picture beside them has none. Before 1.21.4 a style can't
 * say "no shadow" (Compat.badgeStyle), so the draw of a glyph from one of the Arctic tab fonts (arctic:badge_tab,
 * ...) has its shadow switched off for that glyph. Names keep theirs.
 */
@Mixin(targets = "net.minecraft.client.gui.Font$StringRenderOutput")
abstract class BadgeShadowMixin {
	@Mutable
	@Shadow
	private boolean dropShadow;

	private boolean arctic$shadowWas;

	@Inject(method = "accept(ILnet/minecraft/network/chat/Style;I)Z", at = @At("HEAD"))
	private void arctic$noShadowIn(int index, Style style, int codepoint, CallbackInfoReturnable<Boolean> cir) {
		arctic$shadowWas = dropShadow;
		if (dropShadow && arctic$tabFont(style)) {
			dropShadow = false;
		}
	}

	@Inject(method = "accept(ILnet/minecraft/network/chat/Style;I)Z", at = @At("RETURN"))
	private void arctic$noShadowOut(int index, Style style, int codepoint, CallbackInfoReturnable<Boolean> cir) {
		dropShadow = arctic$shadowWas;
	}

	private static boolean arctic$tabFont(Style style) {
		net.minecraft.resources.Identifier font = style.getFont();
		return font != null && com.arcticlauncher.mod.ArcticMod.ID.equals(font.getNamespace()) && font.getPath().endsWith("_tab");
	}
}
//#endif
