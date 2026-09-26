package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.mod.GfxImpl;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/** Vanilla text fields: Arctic field background. */
@Mixin(EditBox.class)
abstract class EditBoxMixin {
	@Redirect(
			method = "extractWidgetRenderState",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Lcom/mojang/renderpearl/api/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIII)V"))
	private void arctic$field(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h) {
		if (!ArcticClient.restyles()) {
			g.blitSprite(pipeline, sprite, x, y, w, h);
			return;
		}
		Skin.field(new GfxImpl(g), ArcticClient.style(), x, y, w, h, ((AbstractWidget) (Object) this).isFocused());
	}
}
