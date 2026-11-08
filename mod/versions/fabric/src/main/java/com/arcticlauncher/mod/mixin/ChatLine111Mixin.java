package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.11 && MC < 26.1
import com.arcticlauncher.mod.FancyFontMetrics;
import com.arcticlauncher.mod.SmoothFont;
import net.minecraft.client.gui.components.ChatComponent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.ModifyConstant;

/**
 * 1.21.11 draws the chat and collects its clicks in one pass, with a hardcoded line of 9 and a pen
 * 8 below the top. Inter is taller. Both numbers come from the font when Fancy is on.
 */
@Mixin(ChatComponent.class)
abstract class ChatLine111Mixin {
	@ModifyConstant(
			method = "render(Lnet/minecraft/client/gui/components/ChatComponent$ChatGraphicsAccess;IIZ)V",
			constant = @Constant(intValue = 9))
	private int arctic$line(int vanilla) {
		return SmoothFont.on() ? FancyFontMetrics.lineHeight() : vanilla;
	}

	@ModifyConstant(
			method = "render(Lnet/minecraft/client/gui/components/ChatComponent$ChatGraphicsAccess;IIZ)V",
			constant = @Constant(doubleValue = 8.0))
	private double arctic$pen(double vanilla) {
		return SmoothFont.on() ? FancyFontMetrics.lineHeight() - 1 : vanilla;
	}

	@ModifyConstant(method = "getLineHeight", constant = @Constant(doubleValue = 9.0))
	private double arctic$page(double vanilla) {
		return SmoothFont.on() ? FancyFontMetrics.lineHeight() : vanilla;
	}
}
//#endif
