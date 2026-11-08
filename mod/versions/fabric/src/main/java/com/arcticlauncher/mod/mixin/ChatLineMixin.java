package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.mod.ArcticPacks;
import com.arcticlauncher.mod.FancyFontMetrics;
import net.minecraft.client.gui.components.ChatComponent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.ModifyConstant;

/**
 * Chat draws and collects clicks in one pass, but the line box is a hardcoded 9 and the pen sits 8
 * below its top. Inter's own line is taller, so a click on the top of a letter lands on the line
 * above. Both numbers come from the font when the smooth font is on; the spacing option still
 * scales them.
 */
@Mixin(ChatComponent.class)
abstract class ChatLineMixin {
	@ModifyConstant(
			method = "extractRenderState(Lnet/minecraft/client/gui/components/ChatComponent$ChatGraphicsAccess;IILnet/minecraft/client/gui/components/ChatComponent$DisplayMode;)V",
			constant = @Constant(intValue = 9))
	private int arctic$line(int vanilla) {
		return ArcticPacks.smoothFontOn() ? FancyFontMetrics.lineHeight() : vanilla;
	}

	@ModifyConstant(
			method = "extractRenderState(Lnet/minecraft/client/gui/components/ChatComponent$ChatGraphicsAccess;IILnet/minecraft/client/gui/components/ChatComponent$DisplayMode;)V",
			constant = @Constant(doubleValue = 8.0))
	private double arctic$pen(double vanilla) {
		return ArcticPacks.smoothFontOn() ? FancyFontMetrics.lineHeight() - 1 : vanilla;
	}

	/** How many lines fit. The same line box the drawing uses. */
	@ModifyConstant(method = "getLineHeight", constant = @Constant(doubleValue = 9.0))
	private double arctic$page(double vanilla) {
		return ArcticPacks.smoothFontOn() ? FancyFontMetrics.lineHeight() : vanilla;
	}
}
//#endif
