package com.arcticlauncher.mod.mixin;
//#if MC >= 1.21
import com.arcticlauncher.mod.GlassCtm;
import com.arcticlauncher.mod.GlassSodium;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Pseudo;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
@Pseudo
@Mixin(targets = "net.caffeinemc.mods.sodium.client.render.chunk.compile.pipeline.BlockRenderer", remap = false)
abstract class GlassSodiumMixin {
	@Inject(method = "renderModel", at = @At("HEAD"), remap = false)
	private void arctic$begin(CallbackInfo ci, @Local(argsOnly = true) BlockState state, @Local(argsOnly = true, ordinal = 0) BlockPos pos) { GlassSodium.renderer(this, state, pos); }
	@Inject(method = "renderModel", at = @At("RETURN"), remap = false)
	private void arctic$end(CallbackInfo ci) { GlassCtm.end(); }
}
//#endif
