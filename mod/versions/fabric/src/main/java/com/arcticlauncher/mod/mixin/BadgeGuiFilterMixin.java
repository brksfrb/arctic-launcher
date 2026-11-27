package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.11
import com.arcticlauncher.mod.BadgeTexture;
//#if MC >= 26.3
import com.mojang.renderpearl.api.textures.FilterMode;
//#else
import com.mojang.blaze3d.textures.FilterMode;
//#endif
import net.minecraft.client.gui.font.TextRenderable;
//#if MC >= 26.1
import net.minecraft.client.renderer.state.gui.GlyphRenderState;
//#else
import net.minecraft.client.gui.render.state.GlyphRenderState;
//#endif
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/** GUI glyphs bind their own sampler independently of the registered texture. */
@Mixin(GlyphRenderState.class)
abstract class BadgeGuiFilterMixin {
	@Shadow @Final private TextRenderable renderable;

	@ModifyArg(method = "textureSetup", at = @At(value = "INVOKE", target =
			//#if MC >= 26.3
			"Lcom/mojang/blaze3d/systems/SamplerCache;getClampToEdge(Lcom/mojang/renderpearl/api/textures/FilterMode;)Lcom/mojang/renderpearl/api/textures/GpuSampler;"
			//#else
			"Lcom/mojang/blaze3d/systems/SamplerCache;getClampToEdge(Lcom/mojang/blaze3d/textures/FilterMode;)Lcom/mojang/blaze3d/textures/GpuSampler;"
			//#endif
	))
	private FilterMode arctic$badgeFilter(FilterMode filter) {
		return BadgeTexture.isBadgeView(renderable.textureView()) ? FilterMode.LINEAR : filter;
	}
}
//#endif
