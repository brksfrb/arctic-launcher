package com.arcticlauncher.legacy.mixin;
import com.arcticlauncher.legacy.LegacyGlassCtm;
import com.arcticlauncher.legacy.GlassQuad;
import com.arcticlauncher.legacy.ArcticLegacy;
import java.util.concurrent.atomic.AtomicBoolean;
import net.minecraft.client.render.model.BakedQuad;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
@Mixin(BakedQuad.class)
abstract class GlassLegacyBakedQuadMixin implements GlassQuad {
	@Unique private LegacyGlassCtm.Variants arctic$variants;
	@Unique private static final AtomicBoolean arctic$ctmFailureLogged = new AtomicBoolean();
	@Inject(method = "<init>", at = @At("RETURN"))
	private void arctic$bake(CallbackInfo ci) {
		try {
			arctic$variants = LegacyGlassCtm.prepare((BakedQuad) (Object) this);
		} catch (RuntimeException e) {
			arctic$variants = null;
			if (arctic$ctmFailureLogged.compareAndSet(false, true))
				ArcticLegacy.LOG.warn("Connected glass could not prepare a quad; using its original appearance", e);
		}
	}
	@Override public LegacyGlassCtm.Variants arctic$ctm() { return arctic$variants; }
}
