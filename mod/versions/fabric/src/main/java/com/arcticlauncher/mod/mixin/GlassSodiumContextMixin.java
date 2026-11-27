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
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
@Pseudo
@Mixin(targets = "me.jellysquid.mods.sodium.client.render.chunk.compile.pipeline.BlockRenderer", remap = false)
abstract class GlassSodiumContextMixin {
	// Both Sodium 0.4 and 0.5 support MC 1.20.1. Select by the renderer's signature.
	private static final String RENDER = "renderModel(Lme/jellysquid/mods/sodium/client/render/chunk/compile/pipeline/BlockRenderContext;";
	private static final String RENDER_04 = RENDER + "Lme/jellysquid/mods/sodium/client/render/chunk/compile/buffers/ChunkModelBuilder;)Z";
	private static final String RENDER_05 = RENDER + "Lme/jellysquid/mods/sodium/client/render/chunk/compile/ChunkBuildBuffers;)V";

	@Inject(method = RENDER_04, at = @At("HEAD"), remap = false, require = 0, expect = 0)
	private void arctic$begin04(@Coerce Object context, @Coerce Object buffers, CallbackInfoReturnable<Boolean> ci) { GlassSodium.context(context); }
	@Inject(method = RENDER_04, at = @At("RETURN"), remap = false, require = 0, expect = 0)
	private void arctic$end04(CallbackInfoReturnable<Boolean> ci) { GlassCtm.end(); }

	@Inject(method = RENDER_05, at = @At("HEAD"), remap = false, require = 0, expect = 0)
	private void arctic$begin(@Coerce Object context, @Coerce Object buffers, CallbackInfo ci) { GlassSodium.context(context); }
	@Inject(method = RENDER_05, at = @At("RETURN"), remap = false, require = 0, expect = 0)
	private void arctic$end(CallbackInfo ci) { GlassCtm.end(); }
}
//#endif
