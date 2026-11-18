package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.together.HostedBanCommands;
import com.arcticlauncher.client.together.WorldBanStore;
import com.mojang.authlib.GameProfile;
import java.io.IOException;
import java.net.SocketAddress;
import java.nio.file.Path;
import net.minecraft.server.integrated.IntegratedPlayerManager;
import net.minecraft.server.integrated.IntegratedServer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(IntegratedPlayerManager.class)
abstract class HostedBanLoginMixin implements WorldBanStore.Holder {
	@Unique
	private WorldBanStore arctic$bans;

	@Override
	public synchronized WorldBanStore arctic$worldBans() throws IOException {
		if (arctic$bans == null) {
			IntegratedServer server = ((IntegratedPlayerManager) (Object) this).getServer();
			Path folder = server.getWorld(0).getSaveHandler().getWorldFolder().toPath();
			arctic$bans = new WorldBanStore(folder);
		}
		return arctic$bans;
	}

	@Inject(method = "checkCanJoin", at = @At("HEAD"), cancellable = true)
	private void arctic$checkWorldBans(SocketAddress remote, GameProfile profile, CallbackInfoReturnable<String> cir) {
		String message = HostedBanCommands.loginMessage(this, profile.getId(), profile.getName(), remote);
		if (message != null) {
			cir.setReturnValue(message);
		}
	}
}
