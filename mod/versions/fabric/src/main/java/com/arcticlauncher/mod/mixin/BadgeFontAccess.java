package com.arcticlauncher.mod.mixin;

import net.minecraft.client.gui.Font;
//#if MC >= 1.21.9
import net.minecraft.client.gui.GlyphSource;
import net.minecraft.network.chat.FontDescription;
//#else
import net.minecraft.client.gui.font.FontSet;
//#if MC >= 1.16
import net.minecraft.resources.Identifier;
//#endif
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;
import org.spongepowered.asm.mixin.gen.Invoker;

@Mixin(Font.class)
public interface BadgeFontAccess {
	//#if MC >= 1.21.9
	@Invoker("getGlyphSource")
	GlyphSource arctic$glyphSource(FontDescription font);
	//#elif MC >= 1.16
	@Invoker("getFontSet")
	FontSet arctic$fontSet(Identifier font);
	//#else
	@Accessor("fonts")
	FontSet arctic$fontSet();
	//#endif
}
