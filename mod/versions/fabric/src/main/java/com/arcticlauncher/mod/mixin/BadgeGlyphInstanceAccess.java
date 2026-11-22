package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.4
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/**
 * GlyphInstance is a private record on 1.21.4+. Invokers let snap read x/y without naming that type
 * in Java source (which would not compile).
 */
//#if MC >= 1.21.9
@Mixin(targets = "net.minecraft.client.gui.font.glyphs.BakedSheetGlyph$GlyphInstance")
//#else
@Mixin(targets = "net.minecraft.client.gui.font.glyphs.BakedGlyph$GlyphInstance")
//#endif
interface BadgeGlyphInstanceAccess {
	@Invoker("x")
	float arctic$x();

	@Invoker("y")
	float arctic$y();
}
//#endif
