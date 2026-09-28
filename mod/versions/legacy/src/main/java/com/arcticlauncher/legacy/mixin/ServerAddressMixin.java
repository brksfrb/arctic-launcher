package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.net.ProxyRoutes;
import net.minecraft.network.ServerAddress;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * An address without a port meant a DNS SRV query. SRV can't go through
 * SOCKS, and asking locally would reveal the server: through a proxy, skip
 * it (the default port; servers that need SRV need their port).
 */
@Mixin(ServerAddress.class)
abstract class ServerAddressMixin {
	private static final int DEFAULT_PORT = 25565;

	@Inject(method = "resolveSrv", at = @At("HEAD"), cancellable = true)
	private static void arctic$noSrv(String host, CallbackInfoReturnable<String[]> cir) {
		if (ProxyRoutes.routes(host)) {
			cir.setReturnValue(new String[] {host, Integer.toString(DEFAULT_PORT)});
		}
	}
}
