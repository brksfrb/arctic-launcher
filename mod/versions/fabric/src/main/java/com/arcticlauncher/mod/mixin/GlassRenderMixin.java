package com.arcticlauncher.mod.mixin;
import com.arcticlauncher.mod.GlassCtm;
import net.minecraft.client.renderer.block.ModelBlockRenderer;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.block.state.BlockState;
//#if MC >= 26.1
import net.minecraft.client.renderer.block.BlockAndTintGetter;
import net.minecraft.client.renderer.block.BlockQuadOutput;
//#elif MC >= 1.15
import net.minecraft.world.level.BlockAndTintGetter;
//#else
import net.minecraft.world.level.BlockAndBiomeGetter;
//#endif
//#if MC >= 1.21.5 && MC < 26.1
import java.util.List;
import net.minecraft.client.renderer.block.model.BlockModelPart;
//#elif MC < 1.21.5
import net.minecraft.client.resources.model.BakedModel;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
@Mixin(ModelBlockRenderer.class)
abstract class GlassRenderMixin {
	@Inject(method = "tesselateBlock", at = @At("HEAD"))
	//#if MC >= 1.19.3
	private void arctic$begin(CallbackInfo ci,
	//#else
	private void arctic$begin(CallbackInfoReturnable<Boolean> ci,
	//#endif
			//#if MC >= 1.15
			@com.llamalad7.mixinextras.sugar.Local(argsOnly = true) BlockAndTintGetter world,
			//#else
			@com.llamalad7.mixinextras.sugar.Local(argsOnly = true) BlockAndBiomeGetter world,
			//#endif
			@com.llamalad7.mixinextras.sugar.Local(argsOnly = true) BlockState state,
			@com.llamalad7.mixinextras.sugar.Local(argsOnly = true) BlockPos pos) {
		GlassCtm.begin(world, state, pos);
	}
	@Inject(method = "tesselateBlock", at = @At("RETURN"))
	//#if MC >= 1.19.3
	private void arctic$end(CallbackInfo ci) { GlassCtm.end(); }
	//#else
	private void arctic$end(CallbackInfoReturnable<Boolean> ci) { GlassCtm.end(); }
	//#endif
}
