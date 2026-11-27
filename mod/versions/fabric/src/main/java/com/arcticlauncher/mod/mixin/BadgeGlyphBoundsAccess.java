package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.9
import net.minecraft.client.gui.font.glyphs.BakedSheetGlyph;
//#else
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

//#if MC >= 1.21.9
@Mixin(BakedSheetGlyph.class)
//#else
@Mixin(BakedGlyph.class)
//#endif
public interface BadgeGlyphBoundsAccess {
	@Accessor("up")
	float arctic$top();

	@Accessor("down")
	float arctic$bottom();
}
