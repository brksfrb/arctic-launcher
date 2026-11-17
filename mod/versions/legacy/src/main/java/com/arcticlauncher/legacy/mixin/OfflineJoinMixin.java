package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.together.OfflineJoin;
import com.mojang.authlib.GameProfile;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.player.ServerPlayerEntity;
import net.minecraft.network.ClientConnection;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.integrated.IntegratedServer;
import net.minecraft.server.network.ServerLoginNetworkHandler;
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
@Mixin(ServerLoginNetworkHandler.class)
abstract class OfflineJoinMixin {
	@Shadow
	@Final
	private MinecraftServer server;

	@Shadow
	@Final
	public ClientConnection connection;

	@Shadow
	private GameProfile profile;

	//#if MC >= 1.12
	@Shadow
	public abstract void method_14978(net.minecraft.text.Text reason);
	//#else
	@Shadow
	public abstract void disconnect(String reason);
	//#endif

	/** The joiner was turned away; the login must not carry on. */
	@Unique
	private boolean arctic$refused;

	@Inject(method = "acceptPlayer", at = @At("HEAD"), cancellable = true)
	private void arctic$offlineFriend(CallbackInfo ci) {
		if (arctic$refused) {
			ci.cancel();
			return;
		}
		// Without a UUID the session wasn't verified (the game gives such a joiner the offline one).
		if (profile == null || profile.isComplete() || !(server instanceof IntegratedServer)) {
			return;
		}
		ClientConfig config = ArcticClient.config();
		SocketAddress remote = connection.getAddress();
		boolean loopback = remote instanceof InetSocketAddress && OfflineJoin.loopback(((InetSocketAddress) remote).getAddress());
		List<String> online = new ArrayList<>();
		for (ServerPlayerEntity player : server.getPlayerManager().getPlayers()) {
			online.add(player.getGameProfile().getName());
		}
		OfflineJoin.Verdict verdict = OfflineJoin.decide(config != null && config.allowOfflineFriends, true, loopback, false,
				profile.getName(), MinecraftClient.getInstance().getSession().getUsername(), online);
		if (verdict == OfflineJoin.Verdict.SAME_NAME) {
			arctic$refused = true;
			String message = Lang.t("social.that_name_is_already_in_this_world");
			//#if MC >= 1.12
			method_14978(new net.minecraft.text.LiteralText(message));
			//#else
			disconnect(message);
			//#endif
			ci.cancel();
		}
	}
}
