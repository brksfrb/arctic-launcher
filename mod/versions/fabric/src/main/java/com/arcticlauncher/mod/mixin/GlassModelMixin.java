package com.arcticlauncher.mod.mixin;
import com.arcticlauncher.mod.GlassCtm;
import java.util.List;
//#if MC >= 26.1
import net.minecraft.client.resources.model.geometry.QuadCollection;
import net.minecraft.client.resources.model.geometry.BakedQuad;
//#elif MC >= 1.21.5
import net.minecraft.client.resources.model.QuadCollection;
import net.minecraft.client.renderer.block.model.BakedQuad;
//#else
import net.minecraft.client.resources.model.SimpleBakedModel;
import net.minecraft.client.renderer.block.model.BakedQuad;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#if MC >= 1.21.5
@Mixin(QuadCollection.class)
//#else
@Mixin(SimpleBakedModel.class)
//#endif
abstract class GlassModelMixin {
	@Inject(method = "getQuads", at = @At("RETURN"), cancellable = true)
	private void arctic$glass(CallbackInfoReturnable<List<BakedQuad>> cir) {
		cir.setReturnValue(GlassCtm.quads(cir.getReturnValue()));
	}
}
