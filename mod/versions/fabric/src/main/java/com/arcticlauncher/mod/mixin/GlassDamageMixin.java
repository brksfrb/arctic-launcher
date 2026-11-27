package com.arcticlauncher.mod.mixin;
import com.arcticlauncher.mod.GlassCtm;
//#if MC >= 26.1
import net.minecraft.client.renderer.LevelRenderer;
//#else
import net.minecraft.client.renderer.block.BlockRenderDispatcher;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
/** Keep CTM off while vanilla emits its destroy overlay (submits it in 26.1+). */
//#if MC >= 26.1
@Mixin(LevelRenderer.class)
//#else
@Mixin(BlockRenderDispatcher.class)
//#endif
abstract class GlassDamageMixin {
	//#if MC >= 26.1
	@Inject(method = "submitBlockDestroyAnimation", at = @At("HEAD"))
	//#else
	@Inject(method = "renderBreakingTexture", at = @At("HEAD"))
	//#endif
	private void arctic$damageBegin(CallbackInfo ci) { GlassCtm.beginDamage(); }
	//#if MC >= 26.1
	@Inject(method = "submitBlockDestroyAnimation", at = @At("RETURN"))
	//#else
	@Inject(method = "renderBreakingTexture", at = @At("RETURN"))
	//#endif
	private void arctic$damageEnd(CallbackInfo ci) { GlassCtm.endDamage(); }
}
