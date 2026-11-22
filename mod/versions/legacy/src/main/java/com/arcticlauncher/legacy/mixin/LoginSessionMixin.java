package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.account.AccountSwitcher;
import com.arcticlauncher.client.together.OfflineJoin;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.mojang.authlib.GameProfile;
import com.mojang.authlib.exceptions.InvalidCredentialsException;
import com.mojang.authlib.minecraft.MinecraftSessionService;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientLoginNetworkHandler;
import net.minecraft.network.ClientConnection;
import net.minecraft.client.util.Session;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;

/**
 * "Invalid session" when joining a server (the game's sign-in expired while it ran): a fresh
 * session comes from the launcher and the join is tried once more, instead of asking the player
 * to restart the game. Joining runs on the network thread, so waiting for the launcher here
 * doesn't freeze the game.
 *
 * <p>An offline account joining a server on this PC (Play Together's tunnel ends there) skips the sign-in
 * call: it can only fail, and every try asks Mojang's session servers again.
 */
@Mixin(ClientLoginNetworkHandler.class)
abstract class LoginSessionMixin {
	@Shadow
	@Final
	private ClientConnection connection;

	@WrapOperation(method = "onHello",
			at = @At(value = "INVOKE", remap = false,
					target = "Lcom/mojang/authlib/minecraft/MinecraftSessionService;joinServer(Lcom/mojang/authlib/GameProfile;Ljava/lang/String;Ljava/lang/String;)V"))
	private void arctic$renewSession(MinecraftSessionService service, GameProfile profile, String token, String serverId,
			Operation<Void> original) {
		java.net.SocketAddress remote = connection.getAddress();
		boolean loopback = remote instanceof java.net.InetSocketAddress
				&& OfflineJoin.loopback(((java.net.InetSocketAddress) remote).getAddress());
		if (OfflineJoin.skipJoinAuth(MinecraftClient.getInstance().getSession().getAccessToken(), loopback)) {
			return;
		}
		try {
			original.call(service, profile, token, serverId);
		} catch (RuntimeException e) {
			throw e;
		} catch (Exception e) {
			AccountSwitcher accounts = ArcticClient.accounts();
			if (!(e instanceof InvalidCredentialsException) || accounts == null || !accounts.renewSession()) {
				throw LoginSessionMixin.<RuntimeException>sneaky(e);
			}
			Session session = MinecraftClient.getInstance().getSession();
			original.call(service, session.getProfile(), session.getAccessToken(), serverId);
		}
	}

	/** Rethrows the original checked exception, which the game's own handler catches. */
	@SuppressWarnings("unchecked")
	private static <T extends Throwable> T sneaky(Throwable e) throws T {
		throw (T) e;
	}
}
