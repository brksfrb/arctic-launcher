package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.net.ProxyRoutes;
import java.net.InetAddress;
import java.net.UnknownHostException;
import net.minecraft.client.network.MultiplayerServerListPinger;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The server list's ping, behind a proxy: the name goes to the proxy
 * unresolved, and the old-style fallback ping (its own direct connection)
 * is skipped.
 */
@Mixin(MultiplayerServerListPinger.class)
abstract class ServerListPingerProxyMixin {
	@Redirect(method = "add", at = @At(value = "INVOKE", target = "Ljava/net/InetAddress;getByName(Ljava/lang/String;)Ljava/net/InetAddress;"))
	private InetAddress arctic$lookup(String host) throws UnknownHostException {
		return ProxyRoutes.lookup(host);
	}

	@Inject(method = "ping", at = @At("HEAD"), cancellable = true)
	private void arctic$noDirectPing(CallbackInfo ci) {
		if (ProxyRoutes.current() != null) {
			ci.cancel();
		}
	}
}
