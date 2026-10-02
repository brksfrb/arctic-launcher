package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.GlyphRuns;
import com.arcticlauncher.polonium.NameTagCache;
import com.arcticlauncher.polonium.gpu.GpuBatchesHolder;
import com.arcticlauncher.polonium.gpu.CrowdTags;
import com.arcticlauncher.polonium.gpu.GpuText;
import org.spongepowered.asm.mixin.Unique;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.gui.Font;
import net.minecraft.client.renderer.feature.NameTagFeatureRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Name tags laid out once and reused while they stay the same (see {@link
 * NameTagCache}), and their quads recorded once and replayed (see {@link
 * GlyphRuns}).
 */
@Mixin(NameTagFeatureRenderer.class)
abstract class NameTagFeatureRendererMixin implements GpuBatchesHolder {
	@Unique
	private final GpuText polonium$gpu = new GpuText();

	@Override
	public GpuText polonium$batches() {
		return polonium$gpu;
	}

	@WrapOperation(
			method = "buildGroup",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/feature/NameTagFeatureRenderer;prepareText(Lnet/minecraft/client/gui/Font;Lnet/minecraft/client/renderer/feature/NameTagFeatureRenderer$Submit;)Lnet/minecraft/client/gui/Font$PreparedText;"))
	private Font.PreparedText polonium$cachedLayout(Font font, NameTagFeatureRenderer.Submit tag, Operation<Font.PreparedText> layout) {
		if (CrowdTags.isList(tag.text())) {
			// A crowd's tags, all at once (the stand-in itself lays out as nothing).
			CrowdTags.draw(tag.text(), polonium$gpu, font);
		}
		return NameTagCache.get(tag.text(), tag.x(), tag.y(), tag.color(), tag.backgroundColor(), () -> layout.call(font, tag));
	}

	@WrapOperation(
			method = "buildGroup",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/Font$PreparedText;visit(Lnet/minecraft/client/gui/Font$GlyphVisitor;)V"))
	private void polonium$replayRuns(Font.PreparedText text, Font.GlyphVisitor renderer, Operation<Void> visit) {
		if (!(text instanceof GlyphRuns.Holder)) {
			visit.call(text, renderer);
			return;
		}
		GlyphRendererAccess tag = (GlyphRendererAccess) renderer;
		if (polonium$gpu.capture(text, tag.polonium$displayMode(), tag.polonium$pose(), tag.polonium$lightCoords())) {
			return;
		}
		for (GlyphRuns.Run run : GlyphRuns.runs(text, tag.polonium$displayMode())) {
			renderer.acceptRenderable(run);
		}
	}
}
