package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.notice.Notices;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import org.lwjgl.input.Mouse;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A click on a pop-up's button (Copy on a screenshot) over any screen. */
@Mixin(Screen.class)
abstract class ScreenMouseMixin {
	@Shadow
	public int width;
	@Shadow
	public int height;
	@Shadow
	protected MinecraftClient client;

	@Inject(method = "handleMouse", at = @At("HEAD"), cancellable = true)
	private void arctic$noticeClick(CallbackInfo ci) {
		if (!Mouse.getEventButtonState() || Mouse.getEventButton() != 0 || client == null) {
			return;
		}
		double x = Mouse.getEventX() * width / (double) Math.max(1, client.width);
		double y = height - Mouse.getEventY() * height / (double) Math.max(1, client.height) - 1;
		if (Notices.click(x, y)) {
			ci.cancel();
		}
	}
}
