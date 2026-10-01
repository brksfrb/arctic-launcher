package com.arcticlauncher.mod.mixin;

import net.minecraft.client.gui.screens.multiplayer.ServerSelectionList;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/**
 * The server list pings with 5 threads, and a server that doesn't answer
 * holds one until it times out, so a few dead entries kept the rest of a
 * long list waiting. 16 ping at once.
 */
@Mixin(ServerSelectionList.class)
abstract class ServerPingPoolMixin {
	@ModifyArg(method = "<clinit>", require = 0,
			at = @At(value = "INVOKE", target = "Ljava/util/concurrent/ScheduledThreadPoolExecutor;<init>(ILjava/util/concurrent/ThreadFactory;)V"))
	private static int arctic$morePingers(int threads) {
		return Math.max(threads, 16);
	}
}
