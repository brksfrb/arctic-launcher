//#if MC >= 1.20
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.mod.GfxImpl;
//#if MC >= 26.3
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
//#elif MC >= 1.21.6
import com.mojang.blaze3d.pipeline.RenderPipeline;
//#elif MC >= 1.21.2
import java.util.function.Function;
import net.minecraft.client.renderer.RenderType;
//#endif
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Checkbox;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/** Vanilla checkboxes: Arctic box and check mark. */
@Mixin(Checkbox.class)
abstract class CheckboxMixin {
	//#if MC >= 1.21.2
	@Redirect(
			//#if MC >= 1.21.11
			method = "extractContents",
			//#else
			method = "extractWidgetRenderState",
			//#endif
			at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE_TINTED, ordinal = 0))
	//#if MC >= 1.21.6
	private void arctic$box(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
	//#else
	private void arctic$box(GuiGraphicsExtractor g, Function<Identifier, RenderType> pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
	//#endif
		if (!ArcticClient.restyles()) {
			g.blitSprite(pipeline, sprite, x, y, w, h, color);
			return;
		}
		Checkbox box = (Checkbox) (Object) this;
		Skin.checkbox(new GfxImpl(g), ArcticClient.style(), x, y, Math.min(w, h), box.selected(), box.isHoveredOrFocused());
	}
	//#elif MC >= 1.20.2
	// 1.20.2 - 1.21.1: the checkbox sprite has no atlas lookup or tint at all.
	@Redirect(method = "extractWidgetRenderState", at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE, ordinal = 0))
	private void arctic$box(GuiGraphicsExtractor g, Identifier sprite, int x, int y, int w, int h) {
		if (!ArcticClient.restyles()) {
			g.blitSprite(sprite, x, y, w, h);
			return;
		}
		Checkbox box = (Checkbox) (Object) this;
		Skin.checkbox(new GfxImpl(g), ArcticClient.style(), x, y, Math.min(w, h), box.selected(), box.isHoveredOrFocused());
	}
	//#else
	// Before 1.20.2 the checkbox has its own texture (no sprite atlas): one plain blit.
	@Redirect(
			method = "extractWidgetRenderState",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blit(Lnet/minecraft/resources/Identifier;IIFFIIII)V"))
	private void arctic$box(GuiGraphicsExtractor g, Identifier sprite, int x, int y, float u, float v, int w, int h, int texW, int texH) {
		if (!ArcticClient.restyles()) {
			g.blit(sprite, x, y, u, v, w, h, texW, texH);
			return;
		}
		Checkbox box = (Checkbox) (Object) this;
		Skin.checkbox(new GfxImpl(g), ArcticClient.style(), x, y, Math.min(w, h), box.selected(), box.isHoveredOrFocused());
	}
	//#endif
}
//#endif
