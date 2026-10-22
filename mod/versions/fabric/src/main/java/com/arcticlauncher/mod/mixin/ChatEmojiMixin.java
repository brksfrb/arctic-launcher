//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.EmojiPicker;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.components.EditBox;
//#if MC >= 1.20
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#endif
import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.client.gui.screens.Screen;
//#if MC >= 1.21.9
import net.minecraft.client.input.MouseButtonEvent;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** The emoji button at the end of the chat line, and its box of symbols. */
@Mixin(ChatScreen.class)
abstract class ChatEmojiMixin {
	@Shadow
	protected EditBox input;

	@Inject(method = "extractRenderState", at = @At("TAIL"), require = 0)
	//#if MC >= 1.20
	private void arctic$emoji(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$draw(new GfxImpl(g), mouseX, mouseY);
	}
	//#elif MC >= 1.16
	private void arctic$emoji(com.mojang.blaze3d.vertex.PoseStack pose, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$draw(GfxImpl.of(pose), mouseX, mouseY);
	}
	//#else
	private void arctic$emoji(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$draw(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), mouseX, mouseY);
	}
	//#endif

	private void arctic$draw(GfxImpl g, int mouseX, int mouseY) {
		Screen screen = (Screen) (Object) this;
		if (ArcticClient.config() != null) {
			EmojiPicker.render(g, ArcticClient.config(), mouseX, mouseY, screen.width, screen.height);
		}
	}

	@Inject(method = "removed", at = @At("HEAD"), require = 0)
	private void arctic$closed(CallbackInfo ci) {
		EmojiPicker.closed();
	}

	//#if MC >= 1.21.9
	@Inject(method = "mouseClicked", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$click(MouseButtonEvent event, boolean doubleClick, CallbackInfoReturnable<Boolean> cir) {
		if (event.button() == 0) {
			arctic$handle((int) event.x(), (int) event.y(), cir);
		}
	}
	//#else
	@Inject(method = "mouseClicked", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$click(double x, double y, int button, CallbackInfoReturnable<Boolean> cir) {
		if (button == 0) {
			arctic$handle((int) x, (int) y, cir);
		}
	}
	//#endif

	private void arctic$handle(int x, int y, CallbackInfoReturnable<Boolean> cir) {
		if (ArcticClient.config() == null) {
			return;
		}
		Screen screen = (Screen) (Object) this;
		String symbol = EmojiPicker.click(ArcticClient.config(), x, y, screen.width, screen.height);
		if (symbol != null) {
			if (!symbol.isEmpty()) {
				input.insertText(symbol);
			}
			cir.setReturnValue(true);
		}
	}
}
//#endif
