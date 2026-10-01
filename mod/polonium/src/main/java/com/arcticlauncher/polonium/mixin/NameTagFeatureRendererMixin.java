package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.NameTagCache;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.gui.Font;
import net.minecraft.client.renderer.feature.NameTagFeatureRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** Name tags laid out once and reused while they stay the same (see {@link NameTagCache}). */
@Mixin(NameTagFeatureRenderer.class)
abstract class NameTagFeatureRendererMixin {
	@WrapOperation(
			method = "buildGroup",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/feature/NameTagFeatureRenderer;prepareText(Lnet/minecraft/client/gui/Font;Lnet/minecraft/client/renderer/feature/NameTagFeatureRenderer$Submit;)Lnet/minecraft/client/gui/Font$PreparedText;"))
	private Font.PreparedText polonium$cachedLayout(Font font, NameTagFeatureRenderer.Submit tag, Operation<Font.PreparedText> layout) {
		return NameTagCache.get(tag.text(), tag.x(), tag.y(), tag.color(), tag.backgroundColor(), () -> layout.call(font, tag));
	}
}
