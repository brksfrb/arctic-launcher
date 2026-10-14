package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.gui.screen.DisconnectedScreen;
import net.minecraft.client.gui.screen.multiplayer.MultiplayerScreen;
import net.minecraft.client.gui.widget.ButtonWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Extend the original menus without replacing their screens or existing buttons. */
@Mixin({TitleScreen.class, MultiplayerScreen.class, DisconnectedScreen.class})
abstract class NativeMenuMixin extends Screen {
	@Unique private final java.util.Map<ButtonWidget, Runnable> arctic$actions = new java.util.LinkedHashMap<>();
	@Unique private com.arcticlauncher.client.servers.DisconnectedPage arctic$reconnect;
	@Unique private ButtonWidget arctic$cancel;
	@Inject(method = "init", at = @At("TAIL"))
	private void arctic$init(CallbackInfo ci) {
		arctic$actions.clear(); arctic$reconnect = null; arctic$cancel = null;
		arctic$add("Arctic", () -> ArcticClient.platform().openPage(ArcticClient.arcticMenu()));
		if ((Object) this instanceof TitleScreen) {
			arctic$add("Replays", () -> ArcticClient.platform().openPage(com.arcticlauncher.client.menu.Menus.page("replays")));
			arctic$add("Edit HUD", () -> ArcticClient.platform().openPage(new com.arcticlauncher.client.menu.HudEditor()));
		} else if ((Object) this instanceof MultiplayerScreen) {
			arctic$add("Arctic servers", () -> com.arcticlauncher.legacy.ServerMenus.open((Screen) (Object) this));
		} else {
			arctic$reconnect = new com.arcticlauncher.client.servers.DisconnectedPage("", "");
			if (arctic$reconnect.canReconnect()) {
				arctic$add("Reconnect", arctic$reconnect::reconnect);
				arctic$cancel = arctic$add("Cancel auto", arctic$reconnect::cancelReconnect);
			}
		}
	}
	@Unique private ButtonWidget arctic$add(String label, Runnable action) {
		ButtonWidget button = new ButtonWidget(9500 + arctic$actions.size(), 4, 4, 76, 20, label);
		arctic$actions.put(button, action); buttons.add(button); return button;
	}
	@Inject(method = "buttonClicked", at = @At("HEAD"), cancellable = true)
	private void arctic$click(ButtonWidget button, CallbackInfo ci) {
		Runnable action = arctic$actions.get(button);
		if (action != null) { action.run(); ci.cancel(); }
	}
	@Inject(method = "render", at = @At("HEAD"))
	private void arctic$layout(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		if (arctic$reconnect != null) {
			if (arctic$cancel != null) arctic$cancel.visible = arctic$reconnect.autoOn();
			arctic$reconnect.tickReconnect();
		}
		java.util.List<ButtonWidget> placed = new java.util.ArrayList<>();
		for (ButtonWidget b : buttons) if (!arctic$actions.containsKey(b) && b.visible) placed.add(b);
		for (ButtonWidget button : arctic$actions.keySet()) {
			if (!button.visible) continue;
			boolean found = false;
			for (int y = 4; y + 20 <= height - 4 && !found; y += 24) {
				for (int x = 4; x + 76 <= width - 4; x += 80) {
					boolean free = true;
					for (ButtonWidget b : placed) {
						com.arcticlauncher.legacy.LegacyButtonBounds size = (com.arcticlauncher.legacy.LegacyButtonBounds) b;
						if (x < b.x + size.arctic$width() + 3 && x + 79 > b.x && y < b.y + size.arctic$height() + 3 && y + 23 > b.y) { free = false; break; }
					}
					if (free) { button.x = x; button.y = y; placed.add(button); found = true; break; }
				}
			}
		}
	}
}
