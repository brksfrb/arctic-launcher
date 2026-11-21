package com.arcticlauncher.mod.mixin;

//#if MC >= 1.13
import com.arcticlauncher.mod.BadgeGlyphBounds;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.Shadow;

/**
 * Cached badge glyphs keep their texture, but the quad must cover exactly {@code resolution / ppu}
 * text units so the blit lands on whole framebuffer pixels at the current scale, not at the
 * center of the size bucket.
 */
//#if MC >= 1.21.9
@Mixin(net.minecraft.client.gui.font.glyphs.BakedSheetGlyph.class)
//#else
@Mixin(net.minecraft.client.gui.font.glyphs.BakedGlyph.class)
//#endif
abstract class BadgeGlyphBoundsMixin implements BadgeGlyphBounds {
	@Shadow @Final @Mutable private float left;
	@Shadow @Final @Mutable private float right;
	@Shadow @Final @Mutable private float up;
	@Shadow @Final @Mutable private float down;

	@Override
	public void arctic$setSide(float side, boolean tab) {
		//#if MC < 1.20.5
		float top = tab ? 2 : 3;
		//#else
		float top = tab ? -1 : 0;
		//#endif
		left = 0f;
		right = side;
		up = top;
		down = top + side;
	}
}
//#endif
