package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.Input;
import com.arcticlauncher.mod.PageScreen;
import net.minecraft.client.gui.components.EditBox;
//#if MC >= 1.19.3
import net.minecraft.client.gui.components.MultiLineEditBox;
//#endif
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
//#if MC >= 1.19.3
import net.minecraft.client.gui.screens.inventory.AbstractSignEditScreen;
//#else
import net.minecraft.client.gui.screens.inventory.SignEditScreen;
//#endif
import net.minecraft.client.gui.screens.inventory.BookEditScreen;
//#if MC >= 1.21
import net.minecraft.client.gui.screens.options.controls.KeyBindsScreen;
//#else
import net.minecraft.client.gui.screens.controls.KeyBindsScreen;
//#endif
import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.KeyboardHandler;
//#if MC >= 1.21.9
import net.minecraft.client.input.KeyEvent;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Arctic's keys: while playing, and Right Shift (the Arctic menu) over
 * Minecraft's own screens too, unless you're typing or in an inventory.
 * Arctic's pages get their keys through {@link com.arcticlauncher.mod.PageScreen}.
 */
@Mixin(KeyboardHandler.class)
abstract class KeyboardHandlerMixin {
	//#if MC >= 1.21.9
	@Inject(method = "keyPress", at = @At("HEAD"), cancellable = true)
	private void arctic$key(long window, int action, KeyEvent event, CallbackInfo ci) {
		if (handled(event.key(), action)) {
			ci.cancel();
		}
	}
	//#else
	@Inject(method = "keyPress", at = @At("HEAD"), cancellable = true)
	private void arctic$key(long window, int key, int scancode, int action, int modifiers, CallbackInfo ci) {
		if (handled(key, action)) {
			ci.cancel();
		}
	}
	//#endif

	private static boolean handled(int key, int action) {
		if (action != com.arcticlauncher.mod.compat.KeyCodes.PRESS) {
			return false;
		}
		Screen screen = Compat.screen();
		if (screen == null) {
			return ArcticClient.keyPressed(Input.key(key));
		}
		if (screen instanceof PageScreen || typing(screen)) {
			return false;
		}
		return ArcticClient.screenKeyPressed(Input.key(key));
	}

	/** Screens where Right Shift is typing (or picking a key), not a shortcut. */
	private static boolean typing(Screen screen) {
		GuiEventListener focused = screen.getFocused();
		//#if MC >= 1.19.3
		return focused instanceof EditBox || focused instanceof MultiLineEditBox
				|| screen instanceof ChatScreen || screen instanceof AbstractSignEditScreen
				|| screen instanceof BookEditScreen || screen instanceof KeyBindsScreen
				|| screen instanceof AbstractContainerScreen;
		//#else
		return focused instanceof EditBox
				|| screen instanceof ChatScreen || screen instanceof SignEditScreen
				|| screen instanceof BookEditScreen || screen instanceof KeyBindsScreen
				|| screen instanceof AbstractContainerScreen;
		//#endif
	}
}
