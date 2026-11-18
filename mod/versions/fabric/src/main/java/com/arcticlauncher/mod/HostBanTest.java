//#if MC >= 1.21.9
package com.arcticlauncher.mod;

import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.together.WorldBanStore;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import io.netty.channel.local.LocalAddress;
import java.net.InetSocketAddress;
import java.nio.file.Path;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.client.server.IntegratedServer;
import net.minecraft.commands.CommandSource;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.players.NameAndId;
import net.minecraft.server.players.PlayerList;
import net.minecraft.world.level.storage.LevelResource;

/** Runs on a fresh integrated server with -Pdevelopment_scenes=true and -Darctic.selftest=hostban. */
final class HostBanTest {
	private HostBanTest() {}

	static void start() {
		Minecraft mc = Minecraft.getInstance();
		CompletableFuture.delayedExecutor(5, TimeUnit.SECONDS).execute(() -> mc.execute(() -> {
			mc.options.pauseOnLostFocus = false;
			WorldTest.createTestWorld(() -> {
				IntegratedServer server = mc.getSingleplayerServer();
				server.execute(() -> check(mc, server));
			});
		}));
	}

	private static void check(Minecraft mc, IntegratedServer server) {
		try {
			PlayerList players = server.getPlayerList();
			ServerPlayer host = players.getPlayers().get(0);
			CommandSourceStack source = host.createCommandSourceStack();
			CommandDispatcher<CommandSourceStack> commands = server.getCommands().getDispatcher();
			NameAndId guest = NameAndId.createOffline("ArcticBanProbe");
			NameAndId other = NameAndId.createOffline("OtherBanProbe");
			InetSocketAddress tunnel = new InetSocketAddress("127.0.0.8", 45501);
			InetSocketAddress lan = new InetSocketAddress("192.168.10.12", 45502);
			String reason = "Test reason";
			String message = Lang.t("social.hostban_disconnect", reason);
			Path folder = server.getWorldPath(LevelResource.ROOT);

			require(!commands.getRoot().getChild("ban").canUse(source.withSource(CommandSource.NULL)),
					"host entity without a host command source");
			expectRejected(commands, source, "ban " + Compat.playerName(host), Lang.t("social.hostban_host"));
			expectRejected(commands, source, "ban-ip ::1", Lang.t("social.hostban_tunnel_ip"));
			commands.execute("ban " + guest.name() + " " + reason, source);
			NameAndId recased = NameAndId.createOffline("arcticbanprobe");
			require(message.equals(text(players.canPlayerLogin(tunnel, recased))), "offline case and reason");
			require(players.canPlayerLogin(tunnel, other) == null, "other tunnel player");
			require(players.canPlayerLogin(new LocalAddress("hostban-test"), guest) == null, "host connection");
			require(guest.id().equals(new WorldBanStore(folder).entries().get(0).uuid), "offline UUID in world file");
			require(commands.execute("banlist players", source) == 1, "ban list");
			require(commands.getCompletionSuggestions(commands.parse("pardon Arc", source)).join().getList().stream()
					.anyMatch(suggestion -> suggestion.getText().equals(guest.name())), "pardon suggestion");
			commands.execute("pardon " + recased.name(), source);
			require(players.canPlayerLogin(tunnel, recased) == null, "profile pardon");

			commands.execute("ban-ip 192.168.10.12 " + reason, source);
			require(message.equals(text(players.canPlayerLogin(lan, guest))), "LAN address and reason");
			require(players.canPlayerLogin(tunnel, guest) == null, "tunnel ignores LAN address ban");
			require(commands.execute("banlist ips", source) == 1, "IP ban list");
			commands.execute("pardon-ip 192.168.10.12", source);
			require(players.canPlayerLogin(lan, guest) == null, "IP pardon");
			ArcticMod.LOG.info("hostban: PASSED");
		} catch (Exception | AssertionError ex) {
			ArcticMod.LOG.error("hostban: FAILED", ex);
		} finally {
			mc.execute(mc::stop);
		}
	}

	private static void expectRejected(CommandDispatcher<CommandSourceStack> commands, CommandSourceStack source,
			String command, String message) throws CommandSyntaxException {
		try {
			commands.execute(command, source);
		} catch (CommandSyntaxException ex) {
			require(message.equals(ex.getRawMessage().getString()), command);
			return;
		}
		throw new AssertionError("Command succeeded: " + command);
	}

	private static String text(Component component) {
		return component == null ? null : component.getString();
	}

	private static void require(boolean ok, String step) {
		if (!ok) throw new AssertionError(step);
	}
}
//#endif
