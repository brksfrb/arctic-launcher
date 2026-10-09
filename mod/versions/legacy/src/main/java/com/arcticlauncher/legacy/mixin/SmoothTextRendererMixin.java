package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacySmoothFont;
import net.minecraft.client.font.TextRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArgs;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import com.mojang.blaze3d.platform.GlStateManager;
import org.lwjgl.opengl.GL11;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
import org.spongepowered.asm.mixin.injection.invoke.arg.Args;

/**
 * The smooth font on 1.8.9 - 1.12.2 (see {@link LegacySmoothFont}): characters are drawn and
 * measured from Inter, so colours, formatting codes, underlines and wrapping keep working.
 */
@Mixin(TextRenderer.class)
abstract class SmoothTextRendererMixin {
	@Shadow
	private float x;
	@Shadow
	private float y;

	@Shadow
	public abstract boolean isUnicode();

	@Shadow
	public abstract int getCharWidth(char c);

	/** Blending was off when this string started, so it's on just for the string. */
	private boolean arctic$blendForString;

	/**
	 * A string starts: Inter's edges need blending. Where the game draws text without it (the
	 * scoreboard's lines are white at ~12% alpha, most HUD text), Minecraft's own font ignores the
	 * colour's alpha; so does this, or that text would come out faded. One driver query per string.
	 */
	@ModifyVariable(method = "drawLayer(Ljava/lang/String;FFIZ)I", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private int arctic$blendString(int color) {
		arctic$blendForString = false;
		if (!arctic$smooth() || GL11.glIsEnabled(GL11.GL_BLEND)) {
			return color;
		}
		arctic$blendForString = true;
		GlStateManager.enableBlend();
		GlStateManager.blendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);
		// Below 0.1 alpha the game's alpha test hides the text anyway: leave that as it is.
		return (color >>> 24) >= 26 ? color | 0xFF000000 : color;
	}

	@Inject(method = "drawLayer(Ljava/lang/String;FFIZ)I", at = @At("RETURN"))
	private void arctic$blendStringDone(String text, float x, float y, int color, boolean shadow, CallbackInfoReturnable<Integer> cir) {
		if (arctic$blendForString) {
			arctic$blendForString = false;
			GlStateManager.disableBlend();
		}
	}

	private boolean arctic$smooth() {
		return LegacySmoothFont.on() && !isUnicode();
	}

	/** One character at the pen. */
	@Inject(method = "drawLayer(CZ)F", at = @At("HEAD"), cancellable = true)
	private void arctic$draw(char c, boolean italic, CallbackInfoReturnable<Float> cir) {
		if (!arctic$smooth()) {
			return;
		}
		if (c == ' ') {
			cir.setReturnValue(LegacySmoothFont.advance(c));
			return;
		}
		LegacySmoothFont.Glyph glyph = LegacySmoothFont.glyph(c);
		if (glyph != null) {
			cir.setReturnValue(LegacySmoothFont.draw(glyph, x, y, italic));
		}
	}

	/** Whole GUI pixels, for wrapping and trimming (drawing keeps Inter's exact spacing). */
	@Inject(method = "getCharWidth", at = @At("HEAD"), cancellable = true)
	private void arctic$charWidth(char c, CallbackInfoReturnable<Integer> cir) {
		if (c == '§' || !arctic$smooth()) {
			return;
		}
		float advance = LegacySmoothFont.advance(c);
		if (advance >= 0) {
			cir.setReturnValue(Math.round(advance));
		}
	}

	/** The width the text is drawn at: Inter's exact spacing added up, formatting codes skipped. */
	@Inject(method = "getStringWidth", at = @At("HEAD"), cancellable = true)
	private void arctic$stringWidth(String text, CallbackInfoReturnable<Integer> cir) {
		if (text == null || !arctic$smooth()) {
			return;
		}
		float width = 0;
		boolean bold = false;
		for (int i = 0; i < text.length(); i++) {
			char c = text.charAt(i);
			if (c == '§' && i + 1 < text.length()) {
				char code = Character.toLowerCase(text.charAt(++i));
				if (code == 'l') {
					bold = true;
				} else if (code == 'r' || (code >= '0' && code <= '9') || (code >= 'a' && code <= 'f')) {
					bold = false;
				}
				continue;
			}
			float advance = LegacySmoothFont.advance(c);
			width += advance >= 0 ? advance : Math.max(0, getCharWidth(c));
			if (bold && advance != 0) {
				width += 1;
			}
		}
		cir.setReturnValue((int) Math.ceil(width));
	}

	/** Shadows half a GUI pixel behind the text instead of a whole one (the first draw is the shadow). */
	@ModifyArgs(method = "draw(Ljava/lang/String;FFIZ)I",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/font/TextRenderer;drawLayer(Ljava/lang/String;FFIZ)I", ordinal = 0))
	private void arctic$softShadow(Args args) {
		if (!arctic$smooth()) {
			return;
		}
		float back = 1f - LegacySmoothFont.shadowOffset();
		args.set(1, (float) args.get(1) - back);
		args.set(2, (float) args.get(2) - back);
	}
}
