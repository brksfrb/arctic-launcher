package com.arcticlauncher.mod.mixin;
//#if MC >= 1.17 && MC < 1.21.5
import com.arcticlauncher.mod.GlassCtm;
import com.arcticlauncher.mod.GlassSodium;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Pseudo;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Coerce;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
@Pseudo
@Mixin(targets = "me.jellysquid.mods.sodium.client.render.chunk.compile.pipeline.BlockRenderer", remap = false)
abstract class GlassSodiumContextMixin {
	@Inject(method = "renderModel", at = @At("HEAD"), remap = false)
	private void arctic$begin(@Coerce Object context, @Coerce Object buffers, CallbackInfo ci) { GlassSodium.context(context); }
	@Inject(method = "renderModel", at = @At("RETURN"), remap = false)
	private void arctic$end(CallbackInfo ci) { GlassCtm.end(); }
}
//#endif
