package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.mod.GfxImpl;
//#if MC >= 26.3
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
//#else
import com.mojang.blaze3d.pipeline.RenderPipeline;
//#endif
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
			at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE))
	private void arctic$field(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h) {
		if (!ArcticClient.restyles()) {
			g.blitSprite(pipeline, sprite, x, y, w, h);
			return;
		}
		Skin.field(new GfxImpl(g), ArcticClient.style(), x, y, w, h, ((AbstractWidget) (Object) this).isFocused());
	}
}
