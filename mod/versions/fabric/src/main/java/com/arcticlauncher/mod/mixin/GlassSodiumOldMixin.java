package com.arcticlauncher.mod.mixin;
//#if MC >= 1.16 && MC < 1.21
import com.arcticlauncher.mod.GlassCtm;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.BlockAndTintGetter;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Pseudo;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
@Pseudo
@Mixin(targets = "me.jellysquid.mods.sodium.client.render.pipeline.BlockRenderer", remap = false)
abstract class GlassSodiumOldMixin {
	@Inject(method = "renderModel", at = @At("HEAD"), remap = false)
	private void arctic$begin(CallbackInfoReturnable<Boolean> ci, @Local(argsOnly = true) BlockAndTintGetter world, @Local(argsOnly = true) BlockState state, @Local(argsOnly = true, ordinal = 0) BlockPos pos) { GlassCtm.begin(world, state, pos); }
	@Inject(method = "renderModel", at = @At("RETURN"), remap = false)
	private void arctic$end(CallbackInfoReturnable<Boolean> ci) { GlassCtm.end(); }
}
//#endif
