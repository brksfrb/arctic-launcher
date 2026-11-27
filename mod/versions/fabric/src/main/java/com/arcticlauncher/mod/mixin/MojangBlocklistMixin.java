package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import org.spongepowered.asm.mixin.Mixin;

/** The resolver tests this provider for the original hostname, resolved address, and SRV result. */
@Mixin(targets = "com.mojang.patchy.BlockedServers", remap = false)
abstract class MojangBlocklistMixin {
	//#if MC == 1.17.1
	@WrapMethod(method = "apply(Ljava/lang/String;)Z")
	//#else
	@WrapMethod(method = "test(Ljava/lang/String;)Z")
	//#endif
	private boolean arctic$checkMojangBlocklist(String address, Operation<Boolean> original) {
		ClientConfig config = ArcticClient.config();
		return (config == null || !config.ignoreMojangServerBlocklist) && original.call(address);
	}
}
