package com.arcticlauncher.mod.mixin;

//#if MC < 1.21.9
import com.arcticlauncher.mod.SmoothFont;
import com.mojang.blaze3d.font.GlyphProvider;
import java.util.List;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.resources.ResourceLocation;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
//#if MC < 1.19
import com.mojang.blaze3d.font.GlyphInfo;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#endif

/** The smooth font before 26.1: the default font is built with Inter first (see {@link SmoothFont}). */
@Mixin(FontSet.class)
abstract class SmoothFontSetMixin {
	@Shadow
	@Final
	private ResourceLocation name;

	//#if MC >= 1.20.5
	@ModifyVariable(method = "reload(Ljava/util/List;Ljava/util/Set;)V", at = @At("HEAD"), argsOnly = true)
	private List<GlyphProvider.Conditional> arctic$inter(List<GlyphProvider.Conditional> providers) {
		return SmoothFont.isDefault(name) ? SmoothFont.withInter(providers) : providers;
	}
	//#else
	@ModifyVariable(method = "reload(Ljava/util/List;)V", at = @At("HEAD"), argsOnly = true)
	private List<GlyphProvider> arctic$inter(List<GlyphProvider> providers) {
		return SmoothFont.isDefault(name) ? SmoothFont.withInter(providers) : providers;
	}
	//#endif

	//#if MC < 1.19
	/** Before 1.19 a space is always 4 GUI pixels wide, too wide between Inter's letters. */
	private static final GlyphInfo ARCTIC_SPACE = () -> SmoothFont.SPACE;

	//#if MC < 1.16
	@Inject(method = "getGlyphInfo(C)Lcom/mojang/blaze3d/font/GlyphInfo;", at = @At("HEAD"), cancellable = true)
	private void arctic$space(char c, CallbackInfoReturnable<GlyphInfo> cir) {
	//#elif MC < 1.18
	@Inject(method = "getGlyphInfo(I)Lcom/mojang/blaze3d/font/GlyphInfo;", at = @At("HEAD"), cancellable = true)
	private void arctic$space(int c, CallbackInfoReturnable<GlyphInfo> cir) {
	//#else
	@Inject(method = "getGlyphInfoForSpace", at = @At("HEAD"), cancellable = true)
	private void arctic$space(int c, CallbackInfoReturnable<GlyphInfo> cir) {
	//#endif
		if (c == ' ' && SmoothFont.on() && SmoothFont.isDefault(name)) {
			cir.setReturnValue(ARCTIC_SPACE);
		}
	}
	//#endif
}
//#endif
