package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.EmoteComponents;
//#if MC >= 1.16
import net.minecraft.network.chat.FormattedText;
//#else
import net.minecraft.network.chat.Component;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/** Replace only the wrapping argument: GuiMessage.content remains the readable original. */
//#if MC >= 26.1
@Mixin(net.minecraft.client.multiplayer.chat.GuiMessage.class)
//#elif MC >= 1.21.11
@Mixin(net.minecraft.client.GuiMessage.class)
//#else
@Mixin(net.minecraft.client.gui.components.ChatComponent.class)
//#endif
abstract class ChatEmoteLayoutMixin {
	@ModifyArg(
			//#if MC >= 1.21.11
			method = "splitLines",
			//#elif MC >= 1.20.6
			method = "addMessageToDisplayQueue",
			//#elif MC >= 1.19
			method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;ILnet/minecraft/client/GuiMessageTag;Z)V",
			//#else
			method = "addMessage(Lnet/minecraft/network/chat/Component;IIZ)V",
			//#endif
			//#if MC >= 1.16
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/ComponentRenderUtils;wrapComponents(Lnet/minecraft/network/chat/FormattedText;ILnet/minecraft/client/gui/Font;)Ljava/util/List;"), index = 0)
	private FormattedText arctic$layout(FormattedText message) { return EmoteComponents.layout(message); }
	//#else
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/ComponentRenderUtils;wrapComponents(Lnet/minecraft/network/chat/Component;ILnet/minecraft/client/gui/Font;ZZ)Ljava/util/List;"), index = 0)
	private Component arctic$layout(Component message) { return EmoteComponents.layout(message); }
	//#endif
}
