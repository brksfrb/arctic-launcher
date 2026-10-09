package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.9 && MC < 26.1
import com.arcticlauncher.mod.SmoothFont;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.mojang.blaze3d.font.GlyphProvider;
import java.util.List;
import java.util.Set;
import net.minecraft.client.gui.font.FontManager;
import net.minecraft.client.gui.font.FontOption;
import net.minecraft.client.gui.font.FontSet;
//#if MC >= 1.21.11
import net.minecraft.resources.Identifier;
//#else
import net.minecraft.resources.ResourceLocation;
//#endif
import org.spongepowered.asm.mixin.Mixin;

/**
 * The smooth font on 1.21.9 - 1.21.11, where a font no longer knows its own name: the default
 * font is built with Inter first (see {@link SmoothFont}).
 */
@Mixin(FontManager.class)
abstract class SmoothFontManagerMixin {
	//#if MC >= 1.21.11
	@WrapMethod(method = "createFontSet")
	private FontSet arctic$inter(Identifier id, List<GlyphProvider.Conditional> providers, Set<FontOption> options, Operation<FontSet> original) {
		return original.call(id, SmoothFont.isDefault(id) ? SmoothFont.withInter(providers) : providers, options);
	}
	//#else
	@WrapMethod(method = "createFontSet")
	private FontSet arctic$inter(ResourceLocation id, List<GlyphProvider.Conditional> providers, Set<FontOption> options, Operation<FontSet> original) {
		return original.call(id, SmoothFont.isDefault(id) ? SmoothFont.withInter(providers) : providers, options);
	}
	//#endif
}
//#endif
