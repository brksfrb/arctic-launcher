//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.together.HostedBanCommands;
import com.arcticlauncher.client.together.WorldBanStore;
//#if MC < 1.21.9
import com.mojang.authlib.GameProfile;
//#endif
import com.arcticlauncher.mod.Compat;
import java.io.IOException;
import java.net.SocketAddress;
import java.nio.file.Path;
import net.minecraft.client.server.IntegratedPlayerList;
import net.minecraft.client.server.IntegratedServer;
import net.minecraft.network.chat.Component;
//#if MC >= 1.21.9
import net.minecraft.server.players.NameAndId;
//#endif
//#if MC >= 1.16
import net.minecraft.world.level.storage.LevelResource;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(IntegratedPlayerList.class)
abstract class HostedBanLoginMixin implements WorldBanStore.Holder {
	@Unique
	private WorldBanStore arctic$bans;

	@Override
	public synchronized WorldBanStore arctic$worldBans() throws IOException {
		if (arctic$bans == null) {
			IntegratedServer server = ((IntegratedPlayerList) (Object) this).getServer();
			//#if MC >= 1.16
			Path folder = server.getWorldPath(LevelResource.ROOT);
			//#else
			Path folder = server.getStorageSource().getBaseDir().resolve(server.getLevelIdName());
			//#endif
			arctic$bans = new WorldBanStore(folder);
		}
		return arctic$bans;
	}

	@Inject(method = "canPlayerLogin", at = @At("HEAD"), cancellable = true)
	private void arctic$checkWorldBans(SocketAddress remote,
			//#if MC >= 1.21.9
			NameAndId profile,
			//#else
			GameProfile profile,
			//#endif
			CallbackInfoReturnable<Component> cir) {
		//#if MC >= 1.21.9
		String message = HostedBanCommands.loginMessage(this, profile.id(), profile.name(), remote);
		//#else
		String message = HostedBanCommands.loginMessage(this, Compat.profileId(profile), Compat.profileName(profile), remote);
		//#endif
		if (message != null) {
			cir.setReturnValue(Compat.literal(message));
		}
	}
}
//#endif
