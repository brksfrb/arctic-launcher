package com.arcticlauncher.mod.mixin;
import com.arcticlauncher.mod.GlassCtm;
import com.arcticlauncher.mod.GlassQuad;
//#if MC >= 26.1
import net.minecraft.client.resources.model.geometry.BakedQuad;
//#else
import net.minecraft.client.renderer.block.model.BakedQuad;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
@Mixin(BakedQuad.class)
abstract class GlassBakedQuadMixin implements GlassQuad {
	@Unique private GlassCtm.Variants arctic$variants;
	@Inject(method = "<init>", at = @At("RETURN"))
	private void arctic$bake(CallbackInfo ci) { arctic$variants = GlassCtm.prepare((BakedQuad) (Object) this); }
	@Override public GlassCtm.Variants arctic$ctm() { return arctic$variants; }
}
