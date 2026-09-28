package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.net.ProxyRoutes;
import java.net.InetAddress;
import java.net.UnknownHostException;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * Joining a server looked its name up here, before connecting: through a
 * proxy it isn't looked up at all (the proxy does it), so the local DNS
 * never learns where you're going.
 */
@Mixin(targets = "net.minecraft.client.gui.screen.ConnectScreen$1")
abstract class ServerLookupMixin {
	@Redirect(method = "run", at = @At(value = "INVOKE", target = "Ljava/net/InetAddress;getByName(Ljava/lang/String;)Ljava/net/InetAddress;"))
	private InetAddress arctic$lookup(String host) throws UnknownHostException {
		return ProxyRoutes.lookup(host);
	}
}
