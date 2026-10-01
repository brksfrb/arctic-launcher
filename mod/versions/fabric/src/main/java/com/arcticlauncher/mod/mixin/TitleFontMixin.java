package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.mod.ArcticPacks;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Titles and subtitles are drawn 4× and 2× larger than chat text; with the
 * smooth font on they use copies of it rasterized that large, so they come
 * out sharp instead of blocky. See {@link ArcticPacks#sharpScaled}.
 */
//#if MC >= 26.2
@Mixin(net.minecraft.client.gui.Hud.class)
//#else
@Mixin(net.minecraft.client.gui.Gui.class)
//#endif
abstract class TitleFontMixin {
	@ModifyVariable(method = "setTitle", at = @At("HEAD"), argsOnly = true)
	private Component arctic$sharpTitle(Component title) {
		return ArcticPacks.sharpScaled(title, ArcticPacks.TITLE_FONT);
	}

	@ModifyVariable(method = "setSubtitle", at = @At("HEAD"), argsOnly = true)
	private Component arctic$sharpSubtitle(Component subtitle) {
		return ArcticPacks.sharpScaled(subtitle, ArcticPacks.SUBTITLE_FONT);
	}
}
//#endif
