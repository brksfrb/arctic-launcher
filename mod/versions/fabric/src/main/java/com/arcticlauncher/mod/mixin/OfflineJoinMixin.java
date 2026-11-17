package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.together.OfflineJoin;
import com.arcticlauncher.mod.Compat;
import com.mojang.authlib.GameProfile;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.server.IntegratedServer;
import net.minecraft.network.Connection;
import net.minecraft.network.chat.Component;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.network.ServerLoginPacketListenerImpl;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Friends who play offline joining through Play Together, an invite or a code. Their joins reach the world
 * from this PC (the tunnel connects to it), and the game already lets a joiner whose session didn't verify
 * into a world on the integrated server as an offline player. The world's sign-in check stays on for
 * everybody; this only refuses an offline joiner who takes the name of the host or of someone in the world.
 */
@Mixin(ServerLoginPacketListenerImpl.class)
abstract class OfflineJoinMixin {
	@Shadow
	@Final
	private MinecraftServer server;

	@Shadow
	@Final
	private Connection connection;

	//#if MC < 1.20.2
	@Shadow
	private GameProfile gameProfile;
	//#endif

	@Shadow
	public abstract void disconnect(Component reason);

	/** The joiner was turned away; the login must not carry on. */
	@Unique
	private boolean arctic$refused;

	//#if MC >= 1.20.2
	@Inject(method = "verifyLoginAndFinishConnectionSetup", at = @At("HEAD"), cancellable = true)
	private void arctic$offlineFriend(GameProfile profile, CallbackInfo ci) {
		// An offline joiner is let in with the game's offline UUID; a verified one has their own.
		boolean offline = Compat.profileId(profile).equals(OfflineJoin.offlineId(Compat.profileName(profile)));
		arctic$check(Compat.profileName(profile), offline, ci);
	}
	//#else
	@Inject(method = "handleAcceptedLogin", at = @At("HEAD"), cancellable = true)
	private void arctic$offlineFriend(CallbackInfo ci) {
		// Without a UUID the session wasn't verified (the game gives such a joiner the offline one).
		boolean offline = gameProfile != null && !gameProfile.isComplete();
		arctic$check(gameProfile == null ? null : Compat.profileName(gameProfile), offline, ci);
	}
	//#endif

	@Unique
	private void arctic$check(String name, boolean offline, CallbackInfo ci) {
		if (arctic$refused) {
			ci.cancel();
			return;
		}
		if (!offline || !(server instanceof IntegratedServer)) {
			return;
		}
		ClientConfig config = ArcticClient.config();
		SocketAddress remote = connection.getRemoteAddress();
		boolean loopback = remote instanceof InetSocketAddress && OfflineJoin.loopback(((InetSocketAddress) remote).getAddress());
		List<String> online = new ArrayList<>();
		for (ServerPlayer player : server.getPlayerList().getPlayers()) {
			online.add(Compat.playerName(player));
		}
		OfflineJoin.Verdict verdict = OfflineJoin.decide(config != null && config.allowOfflineFriends, true, loopback, false,
				name, Minecraft.getInstance().getUser().getName(), online);
		if (verdict == OfflineJoin.Verdict.SAME_NAME) {
			arctic$refused = true;
			disconnect(Compat.literal(Lang.t("social.that_name_is_already_in_this_world")));
			ci.cancel();
		}
	}
}
