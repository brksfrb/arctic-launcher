package com.arcticlauncher.legacy.mixin;
import com.arcticlauncher.legacy.LegacyGlassCtm;
import net.minecraft.world.BlockView;
import net.minecraft.block.BlockState;
import net.minecraft.util.math.BlockPos;
import net.minecraft.client.render.model.BakedModel;
import net.minecraft.client.render.BufferBuilder;
import net.minecraft.client.render.block.BlockModelRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
@Mixin(BlockModelRenderer.class)
abstract class GlassLegacyRenderMixin {
	//#if MC >= 1.9
	@Inject(method = "method_9963", at = @At("HEAD"))
	private void arctic$begin(BlockView world, BakedModel model, BlockState state, BlockPos pos, BufferBuilder buffer, boolean cull, long seed, CallbackInfoReturnable<Boolean> ci) {
	//#else
	@Inject(method = "render(Lnet/minecraft/world/BlockView;Lnet/minecraft/client/render/model/BakedModel;Lnet/minecraft/block/BlockState;Lnet/minecraft/util/math/BlockPos;Lnet/minecraft/client/render/BufferBuilder;Z)Z", at = @At("HEAD"))
	private void arctic$begin(BlockView world, BakedModel model, BlockState state, BlockPos pos, BufferBuilder buffer, boolean cull, CallbackInfoReturnable<Boolean> ci) {
	//#endif
		LegacyGlassCtm.begin(world, state, pos);
	}
	//#if MC >= 1.9
	@Inject(method = "method_9963", at = @At("RETURN"))
	//#else
	@Inject(method = "render(Lnet/minecraft/world/BlockView;Lnet/minecraft/client/render/model/BakedModel;Lnet/minecraft/block/BlockState;Lnet/minecraft/util/math/BlockPos;Lnet/minecraft/client/render/BufferBuilder;Z)Z", at = @At("RETURN"))
	//#endif
	private void arctic$end(CallbackInfoReturnable<Boolean> ci) { LegacyGlassCtm.end(); }
}
