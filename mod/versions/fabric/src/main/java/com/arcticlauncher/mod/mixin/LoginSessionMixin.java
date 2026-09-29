package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.account.AccountSwitcher;
import net.minecraft.client.multiplayer.ClientHandshakePacketListenerImpl;
import net.minecraft.network.chat.Component;
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
 */
@Mixin(ClientHandshakePacketListenerImpl.class)
abstract class LoginSessionMixin {
	@Unique
	private boolean arctic$renewed;

	@Shadow
	private Component authenticateServer(String serverId) {
		throw new AssertionError();
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
