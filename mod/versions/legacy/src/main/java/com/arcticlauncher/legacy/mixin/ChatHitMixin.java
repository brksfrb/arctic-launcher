package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacySmoothFont;
import java.util.List;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.hud.ChatHud;
import net.minecraft.client.gui.hud.ChatHudLine;
import net.minecraft.client.util.Texts;
import net.minecraft.client.util.Window;
import net.minecraft.text.LiteralText;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyConstant;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Chat lines are spaced by a hardcoded 9, and clicks divide by the vanilla font height, which is
 * also 9. Inter's own line (ascent, descent and leading) is taller, so a click on the text lands
 * on the neighbouring line. Drawing and hit-testing share that line height.
 *
 * <p>The click's origin is the same translate the chat is drawn with (2 across, and 48 down from
 * the bottom of the screen then 20 back up). The vanilla click math uses 3 and 27, one pixel off
 * that translate; with the vanilla bitmap that pixel is the empty top of a 9px cell, and with
 * Inter it cuts into the letter.
 */
@Mixin(ChatHud.class)
abstract class ChatHitMixin {
	/** {@code GlStateManager.translate(2, 20)} inside the chat, after the hud's {@code scaledHeight - 48}. */
	private static final int RENDER_X = 2;
	private static final int RENDER_Y = 48 - 20;

	@Shadow
	private int scrolledLines;
	@Shadow
	private List<ChatHudLine> visibleMessages;

	@Shadow
	public abstract boolean isChatFocused();

	@Shadow
	public abstract int getVisibleLineCount();

	@Shadow
	public abstract float getChatScale();

	@Shadow
	public abstract int getWidth();

	@ModifyConstant(method = {"render", "getVisibleLineCount"}, constant = @Constant(intValue = 9))
	private int arctic$line(int vanilla) {
		return LegacySmoothFont.aligned() ? LegacySmoothFont.lineHeight() : vanilla;
	}

	/** The pen sits one pixel below the top of the line, as the vanilla 8 does in a line of 9. */
	@ModifyConstant(method = "render", constant = @Constant(intValue = 8))
	private int arctic$pen(int vanilla) {
		return LegacySmoothFont.aligned() ? LegacySmoothFont.lineHeight() - 1 : vanilla;
	}

	@Redirect(method = "render", at = @At(value = "FIELD", target = "Lnet/minecraft/client/font/TextRenderer;fontHeight:I"))
	private int arctic$height(TextRenderer font) {
		return LegacySmoothFont.aligned() ? LegacySmoothFont.lineHeight() : font.fontHeight;
	}

	@Inject(method = "getTextAt", at = @At("HEAD"), cancellable = true)
	private void arctic$hit(int mouseX, int mouseY, CallbackInfoReturnable<Text> cir) {
		if (!LegacySmoothFont.aligned()) {
			return;
		}
		if (!isChatFocused()) {
			cir.setReturnValue(null);
			return;
		}
		MinecraftClient mc = MinecraftClient.getInstance();
		int scale = new Window(mc).getScaleFactor();
		float chatScale = getChatScale();
		int x = (int) Math.floor((mouseX / (float) scale - RENDER_X) / chatScale);
		int y = (int) Math.floor((mouseY / (float) scale - RENDER_Y) / chatScale);
		if (x < 0 || y < 0) {
			cir.setReturnValue(null);
			return;
		}
		int visible = Math.min(getVisibleLineCount(), visibleMessages.size());
		int lineHeight = LegacySmoothFont.lineHeight();
		if (x > (int) Math.floor(getWidth() / chatScale) || y >= lineHeight * visible) {
			cir.setReturnValue(null);
			return;
		}
		int index = y / lineHeight + scrolledLines;
		if (index < 0 || index >= visibleMessages.size()) {
			cir.setReturnValue(null);
			return;
		}
		int pen = 0;
		TextRenderer font = mc.textRenderer;
		for (Text part : visibleMessages.get(index).getText()) {
			if (!(part instanceof LiteralText)) {
				continue;
			}
			pen += font.getStringWidth(Texts.getRenderChatMessage(((LiteralText) part).getRawString(), false));
			if (pen > x) {
				cir.setReturnValue(part);
				return;
			}
		}
		cir.setReturnValue(null);
	}
}
