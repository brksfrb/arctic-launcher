//#if MC >= 1.20.2
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.svc.SvcPayload;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#if MC >= 1.20.5
import net.minecraft.network.codec.StreamCodec;
//#else
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.protocol.common.ClientboundCustomPayloadPacket;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
//#endif

/** Simple Voice Chat's payloads are read and written as raw bytes (see {@link SvcPayload}). */
//#if MC >= 1.20.5
@Mixin(targets = "net.minecraft.network.protocol.common.custom.CustomPacketPayload$1")
abstract class CustomPayloadCodecMixin {
	@Inject(method = "findCodec", at = @At("HEAD"), cancellable = true)
	private void arctic$svcCodec(Identifier id, CallbackInfoReturnable<StreamCodec<?, ?>> cir) {
		if (SvcPayload.handles(id)) {
			cir.setReturnValue(SvcPayload.codec(id));
		}
	}
}
//#else
// 1.20.2-1.20.4 read an unknown payload into a DiscardedPayload that drops its bytes: keep them for ours.
@Mixin(ClientboundCustomPayloadPacket.class)
abstract class CustomPayloadCodecMixin {
	@Inject(method = "readPayload(Lnet/minecraft/resources/ResourceLocation;Lnet/minecraft/network/FriendlyByteBuf;)Lnet/minecraft/network/protocol/common/custom/CustomPacketPayload;",
			at = @At("HEAD"), cancellable = true)
	private static void arctic$svcPayload(Identifier id, FriendlyByteBuf buf, CallbackInfoReturnable<CustomPacketPayload> cir) {
		if (SvcPayload.handles(id)) {
			cir.setReturnValue(SvcPayload.readFrom(id, buf));
		}
	}
}
//#endif
//#endif
