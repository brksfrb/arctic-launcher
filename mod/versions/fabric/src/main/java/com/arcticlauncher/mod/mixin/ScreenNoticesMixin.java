//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.notice.Notices;
import com.arcticlauncher.mod.GfxImpl;
import com.arcticlauncher.mod.PageScreen;
//#if MC >= 1.20
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#endif
import net.minecraft.client.gui.screens.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Pop-ups (a screenshot just taken, a friend's message) on the screens that cover the game: the connecting
 * and loading screens, the title screen. Without this they wait, unseen, until the game shows again.
 */
@Mixin(Screen.class)
abstract class ScreenNoticesMixin {
	@Inject(method = "extractRenderState", at = @At("TAIL"), require = 0)
	//#if MC >= 1.20
	private void arctic$notices(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$draw(new GfxImpl(g));
	}
	//#elif MC >= 1.16
	private void arctic$notices(com.mojang.blaze3d.vertex.PoseStack pose, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$draw(GfxImpl.of(pose));
	}
	//#else
	private void arctic$notices(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$draw(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()));
	}
	//#endif

	private void arctic$draw(GfxImpl g) {
		Object self = this;
		if (self instanceof PageScreen || ArcticClient.platform() == null || ArcticClient.config() == null) {
			return;
		}
		// In the game the HUD shows them; these screens cover it (loading the world is already "in the game").
		String name = self.getClass().getSimpleName();
		boolean covers = name.equals("ReceivingLevelScreen") || name.equals("LevelLoadingScreen") || name.equals("ProgressScreen");
		if (!ArcticClient.platform().inWorld() || covers) {
			com.arcticlauncher.client.gfx.Draw.fancy = ArcticClient.config().fancy;
			Notices.render(g, ArcticClient.style());
		}
	}
}
//#endif
