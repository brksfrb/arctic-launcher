package com.arcticlauncher.mod.mixin;
//#if MC < 1.19.3
import com.arcticlauncher.client.ctm.GlassTiles;
import com.arcticlauncher.mod.GlassCtm;
import com.arcticlauncher.mod.GlassImages;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#if MC < 1.14
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Final;
import java.util.Set;
import java.util.ArrayList;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#endif
@Mixin(TextureAtlas.class)
abstract class GlassOldAtlasMixin {
	//#if MC >= 1.14
	@Inject(method = "getLoadedSprites", at = @At("RETURN"))
	private void arctic$bind(CallbackInfoReturnable<java.util.List<TextureAtlasSprite>> cir) {
		java.util.Map<Identifier, TextureAtlasSprite> sprites = new java.util.HashMap<>();
		boolean glass = false;
		for (TextureAtlasSprite sprite : cir.getReturnValue()) { sprites.put(sprite.getName(), sprite); glass |= GlassTiles.glass(sprite.getName().toString()); }
		if (glass) GlassCtm.bind(sprites);
	}
	//#endif
	//#if MC >= 1.15
	/** Only the block atlas has glass (particles, paintings and the rest are atlases too). */
	private boolean arctic$blocks() { return ((TextureAtlas) (Object) this).location().equals(TextureAtlas.LOCATION_BLOCKS); }
	//#elif MC >= 1.14
	@org.spongepowered.asm.mixin.Shadow @org.spongepowered.asm.mixin.Final private String path;
	/** 1.14 names atlases by folder: the block atlas is "textures" (particles "textures/particle", ...). */
	private boolean arctic$blocks() { return "textures".equals(path); }
	//#endif
	//#if MC >= 1.14
	@Inject(method = "prepareToStitch", at = @At("HEAD"))
	private void arctic$packs(CallbackInfoReturnable<?> cir, @com.llamalad7.mixinextras.sugar.Local(argsOnly = true) net.minecraft.server.packs.resources.ResourceManager manager) { if (arctic$blocks()) com.arcticlauncher.mod.GlassResources.reload(manager); }
	//#endif
	@ModifyVariable(method = "getResourceLocation", at = @At("HEAD"), argsOnly = true)
	private Identifier arctic$source(Identifier name) { return GlassImages.id(GlassTiles.original(name.toString())); }
	//#if MC >= 1.14
	// Register in the input stream, before vanilla, Fabric and ModernFix take their
	// early-stage snapshots. Adding sprite infos at RETURN is too late for those loaders.
	//#if MC >= 1.15
	@ModifyVariable(method = "prepareToStitch", at = @At("HEAD"), argsOnly = true)
	private java.util.stream.Stream<Identifier> arctic$register(java.util.stream.Stream<Identifier> original) {
		if (!GlassCtm.available() || !arctic$blocks()) return original;
		return original.flatMap(name -> {
			if (!GlassTiles.glass(name.toString())) return java.util.stream.Stream.of(name);
			return java.util.stream.Stream.concat(java.util.stream.Stream.of(name),
				java.util.stream.IntStream.range(0, GlassCtm.tileCount()).mapToObj(i -> GlassImages.id(GlassTiles.variant(name.toString(), i))));
		});
	}
	//#else
	// 1.14 takes the sprite names as an Iterable.
	@ModifyVariable(method = "prepareToStitch", at = @At("HEAD"), argsOnly = true)
	private Iterable<Identifier> arctic$register(Iterable<Identifier> original) {
		if (!GlassCtm.available() || !arctic$blocks()) return original;
		// Lazy, like the stream above: the tile count is set by the pack reload injected at the same HEAD.
		return () -> {
			java.util.List<Identifier> names = new java.util.ArrayList<>();
			for (Identifier name : original) {
				names.add(name);
				if (!GlassTiles.glass(name.toString())) continue;
				for (int i = 0; i < GlassCtm.tileCount(); i++) names.add(GlassImages.id(GlassTiles.variant(name.toString(), i)));
			}
			return names.iterator();
		};
	}
	//#endif
	//#else
	@Shadow @Final private Set<Identifier> sprites;
	@Inject(method = "reload", at = @At("HEAD"))
	private void arctic$tiles(net.minecraft.server.packs.resources.ResourceManager manager, CallbackInfo ci) {
		com.arcticlauncher.mod.GlassResources.reload(manager);
		if (!GlassCtm.available()) return;
		for (Identifier name : new ArrayList<>(sprites)) {
			if (!GlassTiles.glass(name.toString())) continue;
			for (int i = 0; i < GlassCtm.tileCount(); i++) sprites.add(GlassImages.id(GlassTiles.variant(name.toString(), i)));
		}
	}
	//#endif
}
//#endif
