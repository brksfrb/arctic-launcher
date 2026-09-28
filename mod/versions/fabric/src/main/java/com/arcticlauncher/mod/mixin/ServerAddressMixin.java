//#if MC < 1.17
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.net.ProxyRoutes;
import net.minecraft.client.multiplayer.ServerAddress;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Before 1.17 an address without a port meant a DNS SRV query. SRV can't go
 * through SOCKS, and asking locally would reveal the server: through a
 * proxy, skip it (the default port; servers that need SRV need their port).
 */
@Mixin(ServerAddress.class)
abstract class ServerAddressMixin {
	private static final int DEFAULT_PORT = 25565;

	@Inject(method = "lookupSrv", at = @At("HEAD"), cancellable = true)
	//#if MC >= 1.16
	private static void arctic$noSrv(String host, CallbackInfoReturnable<com.mojang.datafixers.util.Pair<String, Integer>> cir) {
		if (ProxyRoutes.routes(host)) {
			cir.setReturnValue(com.mojang.datafixers.util.Pair.of(host, DEFAULT_PORT));
		}
	}
	//#else
	private static void arctic$noSrv(String host, CallbackInfoReturnable<String[]> cir) {
		if (ProxyRoutes.routes(host)) {
			cir.setReturnValue(new String[] {host, Integer.toString(DEFAULT_PORT)});
		}
	}
	//#endif
}
//#endif
