package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.account.AccountSwitcher;
import com.mojang.authlib.GameProfile;
import com.mojang.authlib.exceptions.AuthenticationException;
import com.mojang.authlib.exceptions.InvalidCredentialsException;
import com.mojang.authlib.minecraft.MinecraftSessionService;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientLoginNetworkHandler;
import net.minecraft.client.util.Session;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * Forge flavour of LoginSessionMixin (src/main), as a redirect instead of MixinExtras' wrap: "Invalid
 * session" when joining (the sign-in expired while the game ran) gets a fresh session from the launcher
 * and one more try.
 */
@Mixin(ClientLoginNetworkHandler.class)
abstract class LoginSessionMixin {
	@Redirect(method = "onHello",
			at = @At(value = "INVOKE", remap = false,
					target = "Lcom/mojang/authlib/minecraft/MinecraftSessionService;joinServer(Lcom/mojang/authlib/GameProfile;Ljava/lang/String;Ljava/lang/String;)V"))
	private void arctic$renewSession(MinecraftSessionService service, GameProfile profile, String token, String serverId)
			throws AuthenticationException {
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
