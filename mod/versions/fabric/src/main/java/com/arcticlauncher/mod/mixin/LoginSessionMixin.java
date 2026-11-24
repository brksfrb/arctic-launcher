package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.account.AccountSwitcher;
import com.arcticlauncher.client.account.InvalidSession;
import com.arcticlauncher.client.together.OfflineJoin;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import net.minecraft.client.Minecraft;
import net.minecraft.network.Connection;
import net.minecraft.client.multiplayer.ClientHandshakePacketListenerImpl;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
/** Refresh the Minecraft session before Mojang authentication, or once after a rejection. */
@Mixin(ClientHandshakePacketListenerImpl.class)
abstract class LoginSessionMixin {
	@Shadow
	@Final
	private Connection connection;

	@WrapMethod(method = "authenticateServer")
	private Component arctic$authenticate(String serverId, Operation<Component> original) {
		SocketAddress remote = connection.getRemoteAddress();
		boolean loopback = remote instanceof InetSocketAddress && OfflineJoin.loopback(((InetSocketAddress) remote).getAddress());
		String token = Minecraft.getInstance().getUser().getAccessToken();
		if (OfflineJoin.skipJoinAuth(token, loopback)) {
			return null;
		}
		AccountSwitcher accounts = ArcticClient.accounts();
		if (accounts != null) {
			accounts.refreshBeforeJoin();
		}
		Component error = original.call(serverId);
		// The component retains the invalidSession translation key in every language.
		if (accounts != null && error != null && (InvalidSession.matches(error.toString()) || InvalidSession.matches(error.getString()))
				&& accounts.renewSession()) {
			return original.call(serverId);
		}
		return error;
	}
}
