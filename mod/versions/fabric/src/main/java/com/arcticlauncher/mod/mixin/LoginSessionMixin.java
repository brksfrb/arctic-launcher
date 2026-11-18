package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.account.AccountSwitcher;
import com.arcticlauncher.client.together.OfflineJoin;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import net.minecraft.client.Minecraft;
import net.minecraft.network.Connection;
import net.minecraft.client.multiplayer.ClientHandshakePacketListenerImpl;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * "Invalid session" when joining a server (the game's sign-in expired while
 * it ran): a fresh session comes from the launcher and the join is tried
 * once more, instead of asking the player to restart everything.
 *
 * <p>An offline account joining a server on this PC (Play Together's tunnel ends there) skips the
 * sign-in call: it can only fail, and every try asks Mojang's session servers again.
 */
@Mixin(ClientHandshakePacketListenerImpl.class)
abstract class LoginSessionMixin {
	@Unique
	private boolean arctic$renewed;

	@Shadow
	@Final
	private Connection connection;

	@Shadow
	private Component authenticateServer(String serverId) {
		throw new AssertionError();
	}

	@Inject(method = "authenticateServer", at = @At("HEAD"), cancellable = true)
	private void arctic$offlineFriend(String serverId, CallbackInfoReturnable<Component> cir) {
		SocketAddress remote = connection.getRemoteAddress();
		boolean loopback = remote instanceof InetSocketAddress && OfflineJoin.loopback(((InetSocketAddress) remote).getAddress());
		if (OfflineJoin.skipJoinAuth(Minecraft.getInstance().getUser().getAccessToken(), loopback)) {
			cir.setReturnValue(null);
		}
	}

	@Inject(method = "authenticateServer", at = @At("RETURN"), cancellable = true)
	private void arctic$renewSession(String serverId, CallbackInfoReturnable<Component> cir) {
		Component error = cir.getReturnValue();
		// The message is "disconnect.loginFailedInfo" around
		// "disconnect.loginFailedInfo.invalidSession".
		if (error == null || arctic$renewed || !String.valueOf(error).contains("invalidSession")) {
			return;
		}
		arctic$renewed = true;
		AccountSwitcher accounts = ArcticClient.accounts();
		if (accounts != null && accounts.renewSession()) {
			cir.setReturnValue(authenticateServer(serverId));
		}
	}
}
