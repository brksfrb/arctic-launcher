package com.arcticlauncher.mod.mixin;

//#if MC < 1.21.9
import com.arcticlauncher.client.feature.Badge;
import com.arcticlauncher.client.feature.ChatStampWidth;
import com.arcticlauncher.mod.BadgeTexture;
import com.mojang.blaze3d.font.GlyphInfo;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
//#if MC < 1.16
import com.arcticlauncher.mod.SmoothFont;
//#endif
import net.minecraft.client.gui.font.glyphs.EmptyGlyph;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Old font sets measure the spacer separately from selecting the quad to draw. */
@Mixin(FontSet.class)
abstract class BadgeQuadMixin {
	@Shadow @Final private Identifier name;
	//#if MC >= 1.19
	@Unique private static final GlyphInfo.SpaceGlyphInfo ARCTIC_TAG_ADVANCE = () -> Badge.advance(false);
	@Unique private static final GlyphInfo.SpaceGlyphInfo ARCTIC_TAB_ADVANCE = () -> Badge.advance(true);
	//#else
	@Unique private static final GlyphInfo ARCTIC_TAG_ADVANCE = () -> Badge.advance(false);
	@Unique private static final GlyphInfo ARCTIC_TAB_ADVANCE = () -> Badge.advance(true);
	//#endif
	@Unique private static final BakedGlyph ARCTIC_EMPTY = new EmptyGlyph();
	@Unique private static final GlyphInfo[] ARCTIC_STAMP_ADVANCES = arctic$stampAdvances();

	@Unique
	private static GlyphInfo[] arctic$stampAdvances() {
		GlyphInfo[] glyphs = new GlyphInfo[ChatStampWidth.MAX_POWER - ChatStampWidth.MIN_POWER + 1];
		for (int i = 0; i < glyphs.length; i++) {
			float advance = ChatStampWidth.advance(ChatStampWidth.FIRST + i);
			//#if MC >= 1.19
			GlyphInfo.SpaceGlyphInfo glyph = () -> advance;
			glyphs[i] = glyph;
			//#else
			glyphs[i] = () -> advance;
			//#endif
		}
		return glyphs;
	}

	@Inject(method = "getGlyphInfo", at = @At("HEAD"), cancellable = true)
	private void arctic$advance(
			//#if MC < 1.16
			char codepoint,
			//#else
			int codepoint,
			//#endif
			//#if MC >= 1.19
			boolean filterFishy,
			//#endif
			CallbackInfoReturnable<GlyphInfo> cir) {
		int index = codepoint - ChatStampWidth.FIRST;
		if (index >= 0 && index < ARCTIC_STAMP_ADVANCES.length && com.arcticlauncher.mod.SmoothFont.isDefault(name)) {
			cir.setReturnValue(ARCTIC_STAMP_ADVANCES[index]);
			return;
		}
		int slot = BadgeTexture.slot(codepoint, name);
		if (slot >= 0) cir.setReturnValue(BadgeTexture.tab(slot) ? ARCTIC_TAB_ADVANCE : ARCTIC_TAG_ADVANCE);
		//#if MC < 1.16
		if (SmoothFont.isDefault(name) && (codepoint == BadgeShadow13Mixin.ARCTIC_BLANK || codepoint == BadgeShadow13Mixin.ARCTIC_BLANK_TAG)) {
			cir.setReturnValue(codepoint == BadgeShadow13Mixin.ARCTIC_BLANK ? ARCTIC_TAB_ADVANCE : ARCTIC_TAG_ADVANCE);
		}
		//#endif
	}

	@Inject(method = "getGlyph", at = @At("HEAD"), cancellable = true)
	private void arctic$quad(
			//#if MC < 1.16
			char codepoint,
			//#else
			int codepoint,
			//#endif
			CallbackInfoReturnable<BakedGlyph> cir) {
		if (ChatStampWidth.advance(codepoint) > 0 && com.arcticlauncher.mod.SmoothFont.isDefault(name)) {
			cir.setReturnValue(ARCTIC_EMPTY);
			return;
		}
		int slot = BadgeTexture.slot(codepoint, name);
		if (slot >= 0) cir.setReturnValue(BadgeTexture.quad(slot));
		//#if MC < 1.16
		if (SmoothFont.isDefault(name) && (codepoint == BadgeShadow13Mixin.ARCTIC_BLANK || codepoint == BadgeShadow13Mixin.ARCTIC_BLANK_TAG)) {
			cir.setReturnValue(ARCTIC_EMPTY);
		}
		//#endif
	}
}
//#endif
