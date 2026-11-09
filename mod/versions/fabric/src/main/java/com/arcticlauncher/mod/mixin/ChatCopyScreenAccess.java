//#if MC >= 26.1
package com.arcticlauncher.mod.mixin;

import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.client.gui.components.ChatComponent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(ChatScreen.class)
public interface ChatCopyScreenAccess {
	@Accessor("displayMode")
	ChatComponent.DisplayMode arctic$displayMode();
}
//#endif
