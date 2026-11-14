package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.BlockEntityCulling;
import com.arcticlauncher.legacy.LegacyBlockEntities;
import java.util.List;
import net.minecraft.block.entity.SignBlockEntity;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.render.block.entity.SignBlockEntityRenderer;
import net.minecraft.client.util.Texts;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A sign's lines are laid out (wrapped to the board, every character measured, turned into text with
 * formatting codes) once, not every frame:
 * the layout is kept while the sign has the same lines and the font measures the same, and the game
 * draws it exactly as before.
 */
@Mixin(SignBlockEntityRenderer.class)
abstract class SignTextCacheMixin {
	//#if MC >= 1.12
	private static final String RENDER = "render(Lnet/minecraft/block/entity/SignBlockEntity;DDDFIF)V";
	//#else
	private static final String RENDER = "render(Lnet/minecraft/block/entity/SignBlockEntity;DDDFI)V";
	//#endif

	@Inject(method = RENDER, at = @At("HEAD"))
	private void arctic$sign(SignBlockEntity sign, double x, double y, double z, float tickDelta, int destroyStage,
			//#if MC >= 1.12
			float alpha,
			//#endif
			CallbackInfo ci) {
		LegacyBlockEntities.drawingSign(sign);
	}

	@Inject(method = RENDER, at = @At("RETURN"))
	private void arctic$signDone(SignBlockEntity sign, double x, double y, double z, float tickDelta, int destroyStage,
			//#if MC >= 1.12
			float alpha,
			//#endif
			CallbackInfo ci) {
		LegacyBlockEntities.drawingSign(null);
	}

	@Redirect(method = RENDER, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/util/Texts;wrapLines(Lnet/minecraft/text/Text;ILnet/minecraft/client/font/TextRenderer;ZZ)Ljava/util/List;"))
	private List<Text> arctic$keptLines(Text line, int width, TextRenderer font, boolean trim, boolean forceColor) {
		SignBlockEntity sign = LegacyBlockEntities.sign();
		if (sign == null || !BlockEntityCulling.active(ArcticClient.config())) {
			return Texts.wrapLines(line, width, font, trim, forceColor);
		}
		@SuppressWarnings("unchecked")
		List<Text> kept = (List<Text>) LegacyBlockEntities.SIGNS.get(sign, line);
		if (kept == null) {
			kept = Texts.wrapLines(line, width, font, trim, forceColor);
			LegacyBlockEntities.SIGNS.put(sign, line, kept);
		}
		return kept;
	}

	/** The laid-out line's text with its formatting codes, kept with it. */
	@Redirect(method = RENDER, at = @At(value = "INVOKE", target = "Lnet/minecraft/text/Text;asFormattedString()Ljava/lang/String;"))
	private String arctic$keptText(Text laidOut) {
		SignBlockEntity sign = LegacyBlockEntities.sign();
		if (sign == null || !BlockEntityCulling.active(ArcticClient.config())) {
			return laidOut.asFormattedString();
		}
		String kept = (String) LegacyBlockEntities.SIGN_TEXT.get(sign, laidOut);
		if (kept == null) {
			kept = laidOut.asFormattedString();
			LegacyBlockEntities.SIGN_TEXT.put(sign, laidOut, kept);
		}
		return kept;
	}
}
