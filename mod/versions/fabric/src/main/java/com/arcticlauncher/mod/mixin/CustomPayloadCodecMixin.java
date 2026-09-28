//#if MC >= 1.20.5
package com.arcticlauncher.mod.mixin;

//#if MC >= 1.20.5
import com.arcticlauncher.mod.svc.SvcPayload;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Simple Voice Chat's payloads are read and written as raw bytes (see {@link SvcPayload}). */
@Mixin(targets = "net.minecraft.network.protocol.common.custom.CustomPacketPayload$1")
abstract class CustomPayloadCodecMixin {
	@Inject(method = "findCodec", at = @At("HEAD"), cancellable = true)
	private void arctic$svcCodec(Identifier id, CallbackInfoReturnable<StreamCodec<?, ?>> cir) {
		if (SvcPayload.handles(id)) {
			cir.setReturnValue(SvcPayload.codec(id));
		}
	}
}
//#endif
//#endif
