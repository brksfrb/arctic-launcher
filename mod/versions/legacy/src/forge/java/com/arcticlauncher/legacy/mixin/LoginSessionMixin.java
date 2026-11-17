package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.account.AccountSwitcher;
import com.arcticlauncher.client.together.OfflineJoin;
import com.mojang.authlib.GameProfile;
import com.mojang.authlib.exceptions.AuthenticationException;
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
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * Forge flavour of LoginSessionMixin (src/main), as a redirect instead of MixinExtras' wrap: "Invalid
 * session" when joining (the sign-in expired while the game ran) gets a fresh session from the launcher
 * and one more try. An offline account joining a server on this PC (Play Together's tunnel ends there)
 * skips the sign-in call: it can only fail, and every try asks Mojang's session servers again.
 */
@Mixin(ClientLoginNetworkHandler.class)
abstract class LoginSessionMixin {
	@Shadow
	@Final
	private ClientConnection connection;

	@Redirect(method = "onHello",
			at = @At(value = "INVOKE", remap = false,
					target = "Lcom/mojang/authlib/minecraft/MinecraftSessionService;joinServer(Lcom/mojang/authlib/GameProfile;Ljava/lang/String;Ljava/lang/String;)V"))
	private void arctic$renewSession(MinecraftSessionService service, GameProfile profile, String token, String serverId)
			throws AuthenticationException {
		java.net.SocketAddress remote = connection.getAddress();
		boolean loopback = remote instanceof java.net.InetSocketAddress
				&& OfflineJoin.loopback(((java.net.InetSocketAddress) remote).getAddress());
		if (OfflineJoin.skipJoinAuth(MinecraftClient.getInstance().getSession().getAccessToken(), loopback)) {
			return;
		}
		try {
			service.joinServer(profile, token, serverId);
		} catch (AuthenticationException e) {
			AccountSwitcher accounts = ArcticClient.accounts();
			if (!(e instanceof InvalidCredentialsException) || accounts == null || !accounts.renewSession()) {
				throw e;
			}
			Session session = MinecraftClient.getInstance().getSession();
			service.joinServer(session.getProfile(), session.getAccessToken(), serverId);
		}
	}
}
