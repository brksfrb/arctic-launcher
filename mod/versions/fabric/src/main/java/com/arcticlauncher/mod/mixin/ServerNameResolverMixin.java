//#if MC >= 1.17
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.net.ProxyRoutes;
import java.net.InetSocketAddress;
import java.util.Optional;
import net.minecraft.client.multiplayer.resolver.ResolvedServerAddress;
import net.minecraft.client.multiplayer.resolver.ServerAddress;
import net.minecraft.client.multiplayer.resolver.ServerNameResolver;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Through a proxy, server names aren't looked up here at all (no local DNS
 * or SRV queries that would reveal where you're going); the proxy resolves
 * them. SRV records can't go through SOCKS, so servers that only work via
 * an SRV record need their real port.
 */
@Mixin(ServerNameResolver.class)
abstract class ServerNameResolverMixin {
	@Inject(method = "resolveAddress", at = @At("HEAD"), cancellable = true)
	private void arctic$viaProxy(ServerAddress address, CallbackInfoReturnable<Optional<ResolvedServerAddress>> cir) {
		if (ProxyRoutes.routes(address.getHost())) {
			cir.setReturnValue(Optional.of(ResolvedServerAddress.from(
					InetSocketAddress.createUnresolved(address.getHost(), address.getPort()))));
		}
	}
}
//#endif
