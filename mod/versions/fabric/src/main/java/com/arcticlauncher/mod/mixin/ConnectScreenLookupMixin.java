//#if MC < 1.17
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.net.ProxyRoutes;
import java.net.InetAddress;
import java.net.UnknownHostException;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * Before 1.17 joining a server looked its name up here, before connecting:
 * through a proxy it isn't looked up at all (the proxy does it), so the
 * local DNS never learns where you're going.
 */
@Mixin(targets = "net.minecraft.client.gui.screens.ConnectScreen$1")
abstract class ConnectScreenLookupMixin {
	@Redirect(method = "run", at = @At(value = "INVOKE", target = "Ljava/net/InetAddress;getByName(Ljava/lang/String;)Ljava/net/InetAddress;"))
	private InetAddress arctic$lookup(String host) throws UnknownHostException {
		return ProxyRoutes.lookup(host);
	}
}
//#endif
