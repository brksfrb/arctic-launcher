package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.GlyphRuns;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** Room on each laid-out text for its recorded glyph runs, one set per display mode (see {@link GlyphRuns}). */
@Mixin(targets = "net.minecraft.client.gui.Font$PreparedTextBuilder")
abstract class PreparedTextRunsMixin implements GlyphRuns.Holder {
	@Unique
	private final GlyphRuns.Run[][] polonium$runs = new GlyphRuns.Run[3][];

	@Override
	public GlyphRuns.Run[][] polonium$runs() {
		return polonium$runs;
	}
}
