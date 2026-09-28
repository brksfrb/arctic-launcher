package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.net.ProxyRoutes;
import net.minecraft.client.multiplayer.ServerStatusPinger;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#if MC < 1.17
import java.net.InetAddress;
import java.net.UnknownHostException;
import org.spongepowered.asm.mixin.injection.Redirect;
//#endif

/**
 * The server list's ping, behind a proxy: the old-style ping opens its own
 * direct connection, so it's skipped; before 1.17 the ping also looked the
 * server's name up itself, which the proxy does instead.
 */
@Mixin(ServerStatusPinger.class)
abstract class ServerStatusPingerMixin {
	@Inject(method = "pingLegacyServer", at = @At("HEAD"), cancellable = true)
	private void arctic$noDirectPing(CallbackInfo ci) {
		if (ProxyRoutes.current() != null) {
			ci.cancel();
		}
	}

	//#if MC < 1.17
	@Redirect(method = "pingServer", at = @At(value = "INVOKE", target = "Ljava/net/InetAddress;getByName(Ljava/lang/String;)Ljava/net/InetAddress;"))
	private InetAddress arctic$lookup(String host) throws UnknownHostException {
		return ProxyRoutes.lookup(host);
	}
	//#endif
}
