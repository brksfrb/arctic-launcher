package com.arcticlauncher.legacy.mixin;
import com.arcticlauncher.legacy.LegacyGlassCtm;
import net.minecraft.client.render.block.BlockRenderManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
/** Keep CTM off while vanilla emits its destroy overlay. */
@Mixin(BlockRenderManager.class)
abstract class GlassLegacyDamageMixin {
	@Inject(method = "renderDamage", at = @At("HEAD"))
	private void arctic$damageBegin(CallbackInfo ci) { LegacyGlassCtm.beginDamage(); }
	@Inject(method = "renderDamage", at = @At("RETURN"))
	private void arctic$damageEnd(CallbackInfo ci) { LegacyGlassCtm.endDamage(); }
}
