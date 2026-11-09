package com.arcticlauncher.legacy.mixin;
import com.arcticlauncher.legacy.LegacyGlassCtm;
import com.arcticlauncher.legacy.GlassQuad;
import net.minecraft.client.render.model.BakedQuad;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
@Mixin(BakedQuad.class)
abstract class GlassLegacyBakedQuadMixin implements GlassQuad {
	@Unique private LegacyGlassCtm.Variants arctic$variants;
	@Inject(method = "<init>", at = @At("RETURN"))
	private void arctic$bake(CallbackInfo ci) { arctic$variants = LegacyGlassCtm.prepare((BakedQuad) (Object) this); }
	@Override public LegacyGlassCtm.Variants arctic$ctm() { return arctic$variants; }
}
