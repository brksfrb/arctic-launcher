package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.EmojiPicker;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.screen.ChatScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.TextFieldWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The emoji button at the end of the chat line, and its box of symbols. */
@Mixin(ChatScreen.class)
abstract class ChatEmojiLegacyMixin {
	@Shadow
	protected TextFieldWidget chatField;

	@Inject(method = "render", at = @At("TAIL"))
	private void arctic$emoji(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		Screen screen = (Screen) (Object) this;
		LegacyHooks.drawEmoji(mouseX, mouseY, screen.width, screen.height);
	}

	@Inject(method = "mouseClicked", at = @At("HEAD"), cancellable = true)
	private void arctic$click(int x, int y, int button, CallbackInfo ci) {
		if (button != 0 || ArcticClient.config() == null) {
			return;
		}
		Screen screen = (Screen) (Object) this;
		String symbol = EmojiPicker.click(ArcticClient.config(), x, y, screen.width, screen.height);
		if (symbol != null) {
			if (!symbol.isEmpty()) {
				chatField.write(symbol);
			}
			ci.cancel();
		}
	}
}
