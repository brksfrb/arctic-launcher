package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.Input;
import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.MouseHandler;
//#if MC >= 1.21.9
import net.minecraft.client.input.MouseButtonInfo;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
//#if MC >= 1.21.2
import org.spongepowered.asm.mixin.injection.Redirect;
//#endif
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Counts clicks for the CPS widgets and keeps held buttons; the wheel zooms while zooming. */
@Mixin(MouseHandler.class)
abstract class MouseHandlerMixin {
	//#if MC >= 1.21.9
	@Inject(method = "onButton", at = @At("HEAD"), cancellable = true)
	private void arctic$click(long window, MouseButtonInfo info, int action, CallbackInfo ci) {
		if (noticeClicked(action)) {
			ci.cancel();
			return;
		}
		pressed(info.button(), action);
	}
	//#else
	@Inject(method = "onPress", at = @At("HEAD"), cancellable = true)
	private void arctic$click(long window, int button, int action, int modifiers, CallbackInfo ci) {
		if (noticeClicked(action)) {
			ci.cancel();
			return;
		}
		pressed(button, action);
	}
	//#endif

	/** Scrolling while zoomed zooms (instead of changing the hotbar slot). */
	@Inject(method = "onScroll", at = @At("HEAD"), cancellable = true)
	private void arctic$scroll(long window, double dx, double dy, CallbackInfo ci) {
		if (Compat.screen() == null && com.arcticlauncher.client.replay.Replays.scroll(dy)) {
			ci.cancel();
			return;
		}
		if (Compat.screen() == null && ArcticClient.features() != null && ArcticClient.features().scroll(dy)) {
			ci.cancel();
		}
	}

	/**
	 * Hotbar slot only (1.21.2+ picks the next slot here, after the spectator and screen branches).
	 * Bundle scrolling and every open screen call {@code getNextScrollWheelSelection} on their own.
	 */
	//#if MC >= 1.21.2
	@Redirect(method = "onScroll", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/ScrollWheelHandler;getNextScrollWheelSelection(DII)I"))
	private int arctic$hotbarWrap(double wheel, int current, int limit) {
		com.arcticlauncher.client.config.ClientConfig config = ArcticClient.config();
		if (config == null || config.hotbarScrollWrap || limit != com.arcticlauncher.client.feature.HotbarScroll.SLOTS) {
			return net.minecraft.client.ScrollWheelHandler.getNextScrollWheelSelection(wheel, current, limit);
		}
		return com.arcticlauncher.client.feature.HotbarScroll.wheel(current, wheel, false);
	}
	//#endif

	/** A click on a pop-up's button (Copy on a screenshot) while a screen frees the mouse. */
	private static boolean noticeClicked(int action) {
		if (action != com.arcticlauncher.mod.compat.KeyCodes.PRESS || Compat.screen() == null) {
			return false;
		}
		double[] at = Compat.guiMouse();
		return com.arcticlauncher.client.notice.Notices.click(at[0], at[1]);
	}

	private static void pressed(int button, int action) {
		Compat.mouseButton(button, action != com.arcticlauncher.mod.compat.KeyCodes.RELEASE);
		if (action == com.arcticlauncher.mod.compat.KeyCodes.PRESS && Compat.screen() == null) {
			ArcticClient.mousePressed(Input.mouse(button));
		}
	}
}
