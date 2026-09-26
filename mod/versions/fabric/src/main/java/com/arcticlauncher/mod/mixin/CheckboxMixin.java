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
import net.minecraft.client.gui.components.Checkbox;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/** Vanilla checkboxes: Arctic box and check mark. */
@Mixin(Checkbox.class)
abstract class CheckboxMixin {
	@Redirect(
			method = "extractContents",
			at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE_TINTED, ordinal = 0))
	private void arctic$box(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
		if (!ArcticClient.restyles()) {
			g.blitSprite(pipeline, sprite, x, y, w, h, color);
			return;
		}
		Checkbox box = (Checkbox) (Object) this;
		Skin.checkbox(new GfxImpl(g), ArcticClient.style(), x, y, Math.min(w, h), box.selected(), box.isHoveredOrFocused());
	}
}
