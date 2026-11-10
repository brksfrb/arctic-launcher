package com.arcticlauncher.mod.mixin;
//#if MC < 1.19.3
import com.arcticlauncher.client.ctm.GlassTiles;
import com.arcticlauncher.mod.GlassCtm;
import com.arcticlauncher.mod.GlassImages;
import java.util.ArrayList;
import java.util.Collection;
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
	//#if MC >= 1.14
	@Inject(method = "prepareToStitch", at = @At("HEAD"))
	private void arctic$packs(CallbackInfoReturnable<?> cir, @com.llamalad7.mixinextras.sugar.Local(argsOnly = true) net.minecraft.server.packs.resources.ResourceManager manager) { com.arcticlauncher.mod.GlassResources.reload(manager); }
	//#endif
	@ModifyVariable(method = "getResourceLocation", at = @At("HEAD"), argsOnly = true)
	private Identifier arctic$source(Identifier name) { return GlassImages.id(GlassTiles.original(name.toString())); }
	//#if MC >= 1.15
	@Inject(method = "getBasicSpriteInfos", at = @At("RETURN"), cancellable = true)
	private void arctic$tiles(CallbackInfoReturnable<Collection<TextureAtlasSprite.Info>> cir) {
		if (!GlassCtm.available()) return;
		Collection<TextureAtlasSprite.Info> expanded = new ArrayList<>(cir.getReturnValue());
		for (TextureAtlasSprite.Info info : cir.getReturnValue()) {
			if (!GlassTiles.glass(info.name().toString())) continue;
			for (int i = 0; i < GlassTiles.VARIANTS; i++) expanded.add(new TextureAtlasSprite.Info(GlassImages.id(GlassTiles.variant(info.name().toString(), i)), info.width(), info.height(), ((GlassInfoAccess) (Object) info).arctic$metadata()));
		}
		cir.setReturnValue(expanded);
	}
	//#elif MC >= 1.14
	@Inject(method = "getBasicSpriteInfos", at = @At("RETURN"), cancellable = true)
	private void arctic$tiles(CallbackInfoReturnable<Collection<TextureAtlasSprite>> cir) {
		if (!GlassCtm.available()) return;
		Collection<TextureAtlasSprite> expanded = new ArrayList<>(cir.getReturnValue());
		for (TextureAtlasSprite sprite : cir.getReturnValue()) {
			if (!GlassTiles.glass(sprite.getName().toString())) continue;
			for (int i = 0; i < GlassTiles.VARIANTS; i++) expanded.add(GlassOldSpriteAccess.arctic$new(GlassImages.id(GlassTiles.variant(sprite.getName().toString(), i)), sprite.getWidth(), sprite.getHeight()));
		}
		cir.setReturnValue(expanded);
	}
	//#else
	@Shadow @Final private Set<Identifier> sprites;
	@Inject(method = "reload", at = @At("HEAD"))
	private void arctic$tiles(net.minecraft.server.packs.resources.ResourceManager manager, CallbackInfo ci) {
		com.arcticlauncher.mod.GlassResources.reload(manager);
		if (!GlassCtm.available()) return;
		for (Identifier name : new ArrayList<>(sprites)) {
			if (!GlassTiles.glass(name.toString())) continue;
			for (int i = 0; i < GlassTiles.VARIANTS; i++) sprites.add(GlassImages.id(GlassTiles.variant(name.toString(), i)));
		}
	}
	//#endif
}
//#endif
