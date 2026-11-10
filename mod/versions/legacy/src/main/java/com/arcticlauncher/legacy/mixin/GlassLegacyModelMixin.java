package com.arcticlauncher.legacy.mixin;
import com.arcticlauncher.legacy.LegacyGlassCtm;
import java.util.List;
import net.minecraft.client.render.model.BasicBakedModel;
import net.minecraft.client.render.model.BakedQuad;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
@Mixin(BasicBakedModel.class)
abstract class GlassLegacyModelMixin {
	//#if MC >= 1.9
	@Inject(method = "method_12502", at = @At("RETURN"), cancellable = true)
	//#else
	@Inject(method = {"getQuads", "getByDirection"}, at = @At("RETURN"), cancellable = true)
	//#endif
	private void arctic$quads(CallbackInfoReturnable<List<BakedQuad>> cir) { cir.setReturnValue(LegacyGlassCtm.quads(cir.getReturnValue())); }
}
