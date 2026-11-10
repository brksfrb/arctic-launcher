package com.arcticlauncher.legacy.mixin;
import com.arcticlauncher.client.ctm.GlassTiles;
import com.arcticlauncher.legacy.LegacyGlassCtm;
import java.util.ArrayList;
import java.util.Map;
import net.minecraft.client.texture.Sprite;
import net.minecraft.client.texture.SpriteAtlasTexture;
import net.minecraft.util.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
@Mixin(SpriteAtlasTexture.class)
abstract class GlassLegacyAtlasMixin {
	@Shadow @Final private Map<String, Sprite> spritesToLoad;
	@Shadow @Final private Map<String, Sprite> sprites;
	@Inject(method = "method_7005", at = @At("HEAD"))
	private void arctic$tiles(net.minecraft.resource.ResourceManager manager, CallbackInfo ci) {
		if (!LegacyGlassCtm.available()) return;
		LegacyGlassCtm.packs = com.arcticlauncher.legacy.LegacyGlassResources.load(manager);
		for (String name : new ArrayList<>(spritesToLoad.keySet())) {
			if (!com.arcticlauncher.legacy.LegacyGlassCtm.packs.acceptsSprite(name)) continue;
			for (int i = 0; i < GlassTiles.VARIANTS; i++) {
				String variant = GlassTiles.variant(name, i);
				spritesToLoad.put(variant, new com.arcticlauncher.legacy.LegacyGlassSprite(variant));
			}
		}
	}
	//#if MC >= 1.9
	@Shadow @Final private String name;
	@Inject(method = "method_12487", at = @At("RETURN"), cancellable = true)
	private void arctic$source(Sprite sprite, org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable<Identifier> cir) {
		if (!sprite.getName().startsWith("arctic:ctm/")) return;
		String original = GlassTiles.original(sprite.getName());
		int colon = original.indexOf(':');
		cir.setReturnValue(new Identifier(original.substring(0, colon), name + "/" + original.substring(colon + 1) + ".png"));
	}
	//#else
	@ModifyVariable(method = "method_7003", at = @At("HEAD"), argsOnly = true)
	private Identifier arctic$source(Identifier name) { return new Identifier(GlassTiles.original(name.toString())); }
	//#endif
	@Inject(method = "method_7005", at = @At("RETURN"))
	private void arctic$bind(CallbackInfo ci) { if (LegacyGlassCtm.available()) LegacyGlassCtm.bind(sprites); }
}
