package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyScreenshots;
import java.io.File;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gl.Framebuffer;
import net.minecraft.client.gui.hud.ChatHud;
import net.minecraft.client.util.ScreenshotUtils;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.Slice;

/** Only F2 is asynchronous; screenshot API callers (including self-tests) can still wait for the file. */
@Mixin(MinecraftClient.class)
abstract class ScreenshotKeyMixin {
	@Redirect(method = "handleKeyInput", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/util/ScreenshotUtils;saveScreenshot(Ljava/io/File;IILnet/minecraft/client/gl/Framebuffer;)Lnet/minecraft/text/Text;"))
	private Text arctic$capture(File directory, int width, int height, Framebuffer framebuffer) {
		return LegacyScreenshots.capture(() -> ScreenshotUtils.saveScreenshot(directory, width, height, framebuffer));
	}

	@Redirect(method = "handleKeyInput", slice = @Slice(from = @At(value = "INVOKE", target = "Lnet/minecraft/client/util/ScreenshotUtils;saveScreenshot(Ljava/io/File;IILnet/minecraft/client/gl/Framebuffer;)Lnet/minecraft/text/Text;")), at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/hud/ChatHud;addMessage(Lnet/minecraft/text/Text;)V", ordinal = 0))
	private void arctic$message(ChatHud chat, Text message) {
		LegacyScreenshots.message(message, saved -> MinecraftClient.getInstance().submit(() -> chat.addMessage(saved)));
	}
}
