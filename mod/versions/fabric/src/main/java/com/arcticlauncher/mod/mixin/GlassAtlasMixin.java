package com.arcticlauncher.mod.mixin;
import com.arcticlauncher.mod.GlassCtm;
import java.util.Map;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
@Mixin(TextureAtlas.class)
abstract class GlassAtlasMixin {
	//#if MC < 1.21.9
	@Final
	//#endif
	@Shadow private Map<Identifier, TextureAtlasSprite> texturesByName;
	//#if MC >= 1.19.3
	@Shadow @Final private Identifier location;
	@Inject(method = "upload", at = @At("RETURN"))
	//#elif MC >= 1.15
	@Shadow @Final private Identifier location;
	@Inject(method = "reload", at = @At("RETURN"))
	//#elif MC >= 1.14
	@Inject(method = "reload", at = @At("RETURN"))
	//#else
	@Inject(method = "reload", at = @At("RETURN"))
	//#endif
	private void arctic$bind(CallbackInfo ci) {
		//#if MC >= 1.15
		if (!location.equals(TextureAtlas.LOCATION_BLOCKS)) return;
		//#endif
		if (GlassCtm.available()) GlassCtm.bind(texturesByName);
	}
}
