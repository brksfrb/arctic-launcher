package com.arcticlauncher.mod.mixin;
import com.arcticlauncher.mod.GlassCtm;
import com.arcticlauncher.mod.GlassQuad;
import com.arcticlauncher.mod.ArcticMod;
import java.util.concurrent.atomic.AtomicBoolean;
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
	@Unique private static final AtomicBoolean arctic$ctmFailureLogged = new AtomicBoolean();
	@Inject(method = "<init>", at = @At("RETURN"))
	private void arctic$bake(CallbackInfo ci) {
		try {
			arctic$variants = GlassCtm.prepare((BakedQuad) (Object) this);
		} catch (RuntimeException e) {
			arctic$variants = null;
			if (arctic$ctmFailureLogged.compareAndSet(false, true))
				ArcticMod.LOG.warn("Connected glass could not prepare a quad; using its original appearance", e);
		}
	}
	@Override public GlassCtm.Variants arctic$ctm() { return arctic$variants; }
}
