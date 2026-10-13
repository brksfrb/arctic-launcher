package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ArcticButton;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.screens.DisconnectedScreen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Add Arctic actions to the original screen; never replace or remove a widget owned by another mod. */
@Mixin({TitleScreen.class, JoinMultiplayerScreen.class, DisconnectedScreen.class})
abstract class NativeMenuMixin extends Screen {
	private NativeMenuMixin(Component title) { super(title); }
	@Inject(method = "init", at = @At("TAIL"))
	private void arctic$actions(CallbackInfo ci) {
		ArcticButton.addActions((Screen) (Object) this, button -> addRenderableWidget(button));
	}
}
