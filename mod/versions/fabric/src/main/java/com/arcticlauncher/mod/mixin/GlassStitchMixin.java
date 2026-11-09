package com.arcticlauncher.mod.mixin;
//#if MC >= 1.19.3
import com.arcticlauncher.client.ctm.GlassTiles;
import com.arcticlauncher.mod.GlassCtm;
import com.arcticlauncher.mod.GlassImages;
import com.arcticlauncher.mod.GlassSpriteSource;
import com.arcticlauncher.mod.GlassResources;
import net.minecraft.server.packs.resources.ResourceManager;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.client.renderer.texture.SpriteLoader;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
@Mixin(SpriteLoader.class)
abstract class GlassStitchMixin {
	@org.spongepowered.asm.mixin.Shadow @org.spongepowered.asm.mixin.Final private net.minecraft.resources.Identifier location;
	@Inject(method = "stitch", at = @At("RETURN"))
	private void arctic$bind(CallbackInfoReturnable<SpriteLoader.Preparations> cir) {
		if (location.equals(net.minecraft.client.renderer.texture.TextureAtlas.LOCATION_BLOCKS)) GlassCtm.bind(cir.getReturnValue().regions());
	}
	@Inject(method = "loadAndStitch", at = @At("HEAD"))
	private void arctic$packs(CallbackInfoReturnable<?> cir, @com.llamalad7.mixinextras.sugar.Local(argsOnly = true) ResourceManager manager) { if (location.equals(net.minecraft.client.renderer.texture.TextureAtlas.LOCATION_BLOCKS)) GlassResources.reload(manager); }
	@ModifyVariable(method = "stitch", at = @At("HEAD"), argsOnly = true)
	private List<SpriteContents> arctic$tiles(List<SpriteContents> original) {
		if (!GlassCtm.available()) return original;
		List<SpriteContents> expanded = new ArrayList<>(original);
		for (SpriteContents sprite : original) {
			String name = sprite.name().toString();
			if (!GlassTiles.glass(name)) continue;
			GlassSpriteSource source = (GlassSpriteSource) sprite;
			for (int tile = 0; tile < GlassTiles.COUNT; tile++) {
				expanded.add(source.arctic$variant(GlassImages.id(GlassTiles.variant(name, tile)), GlassImages.generate(source.arctic$image(), sprite.width(), sprite.height(), tile, name)));
			}
		}
		return expanded;
	}
}
//#endif
