//#if MC >= 1.20.2
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.svc.Greeter;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import net.minecraft.network.Connection;
import net.minecraft.resources.Identifier;
import net.neoforged.neoforge.network.registration.NetworkRegistry;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** NeoForge reads the server's channel list (minecraft:register) itself: Arctic says its hellos from it. */
@Mixin(value = NetworkRegistry.class, remap = false)
abstract class NeoForgeRegisterMixin {
	@Inject(method = "onMinecraftRegister", at = @At("TAIL"))
	private static void arctic$greet(Connection connection, Set<Identifier> channels, CallbackInfo ci) {
		List<String> names = new ArrayList<>(channels.size());
		for (Identifier channel : channels) {
			names.add(channel.toString());
		}
		Greeter.greet(connection, names);
	}
}
//#endif
