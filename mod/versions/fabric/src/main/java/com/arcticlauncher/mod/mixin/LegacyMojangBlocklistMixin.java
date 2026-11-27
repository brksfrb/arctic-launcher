package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import io.netty.bootstrap.Bootstrap;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** Mojang's patched Netty checks hostnames and IPs before opening a connection. */
@Mixin(value = Bootstrap.class, remap = false)
abstract class LegacyMojangBlocklistMixin {
	@WrapOperation(method = "checkAddress", at = @At(value = "INVOKE",
			target = "Lcom/mojang/patchy/BlockedServers;isBlockedServer(Ljava/lang/String;)Z"))
	private boolean arctic$checkMojangBlocklist(String address, Operation<Boolean> original) {
		ClientConfig config = ArcticClient.config();
		return (config == null || !config.ignoreMojangServerBlocklist) && original.call(address);
	}
}
