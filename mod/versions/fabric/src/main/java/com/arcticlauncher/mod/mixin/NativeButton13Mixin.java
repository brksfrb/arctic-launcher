//#if MC < 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.NativeMenuButton;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.mod.ArcticButton;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.TitleScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Button.class)
abstract class NativeButton13Mixin {
	@Unique private NativeMenuButton arctic$chrome;
	@Unique private String arctic$label;
	@Unique private String arctic$key;

	@Inject(method = "method_891", at = @At("HEAD"), cancellable = true)
	private void arctic$native(int mx, int my, float delta, CallbackInfo ci) {
		Object screen = Compat.screen();
		if (ArcticClient.style() != Style.ARCTIC || screen == null
				|| screen.getClass() != TitleScreen.class && screen.getClass() != PauseScreen.class) return;
		Button button = (Button) (Object) this;
		if (!button.visible) return;
		String text = Compat.rawLabel(button);
		if (!text.equals(arctic$label)) {
			arctic$label = text;
			arctic$key = Compat.labelKey(button, NativeMenuButton.LABELS);
		}
		if (!ArcticButton.owns(button) && !NativeMenuButton.LABELS.contains(arctic$key)) return;
		if (arctic$chrome == null) arctic$chrome = new NativeMenuButton();
		int w = Compat.widgetWidth(button), h = Compat.widgetHeight(button);
		boolean hovered = mx >= button.x && my >= button.y && mx < button.x + w && my < button.y + h;
		arctic$chrome.draw(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), button.x, button.y, w, h, text,
				"menu.returnToGame".equals(arctic$key) || "menu.singleplayer".equals(arctic$key), button.active, hovered);
		ci.cancel();
	}
}
//#endif
