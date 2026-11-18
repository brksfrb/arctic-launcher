package com.arcticlauncher.legacy;

import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.together.OfflineJoin;
import com.arcticlauncher.client.together.WorldBanStore;
import com.mojang.authlib.GameProfile;
import io.netty.channel.local.LocalAddress;
import java.net.InetSocketAddress;
import java.nio.file.Path;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.player.ServerPlayerEntity;
import net.minecraft.server.PlayerManager;
import net.minecraft.server.command.CommandRegistryProvider;
import net.minecraft.server.integrated.IntegratedServer;

/** Runs on a fresh integrated server with -Pdevelopment_scenes=true and -Darctic.selftest=hostban. */
final class HostBanTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread thread = new Thread(r, "arctic-hostban-test");
		thread.setDaemon(true);
		return thread;
	});

	private HostBanTest() {}

	static void start() {
		MinecraftClient mc = MinecraftClient.getInstance();
		TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> LegacyWorldTest.createTestWorld(() -> {
			IntegratedServer server = mc.getServer();
			server.submit(() -> check(mc, server));
		})), 5, TimeUnit.SECONDS);
	}

	private static void check(MinecraftClient mc, IntegratedServer server) {
		try {
			PlayerManager players = server.getPlayerManager();
			ServerPlayerEntity host = players.getPlayers().get(0);
			CommandRegistryProvider commands = server.getCommandManager();
			GameProfile guest = offline("ArcticBanProbe");
			GameProfile other = offline("OtherBanProbe");
			InetSocketAddress tunnel = new InetSocketAddress("127.0.0.8", 45501);
			InetSocketAddress lan = new InetSocketAddress("192.168.10.12", 45502);
			String reason = "Test reason";
			String message = Lang.t("social.hostban_disconnect", reason);
			Path folder = server.getWorld(0).getSaveHandler().getWorldFolder().toPath();

			require(commands.execute(host, "ban " + host.getGameProfile().getName()) == 0, "host protected");
			require(commands.execute(host, "ban-ip ::1") == 0, "loopback refused");
			require(commands.execute(host, "ban " + guest.getName() + " " + reason) == 1, "ban command");
			GameProfile recased = offline("arcticbanprobe");
			require(message.equals(players.checkCanJoin(tunnel, recased)), "offline case and reason");
			require(players.checkCanJoin(tunnel, other) == null, "other tunnel player");
			require(players.checkCanJoin(new LocalAddress("hostban-test"), guest) == null, "host connection");
			require(guest.getId().equals(new WorldBanStore(folder).entries().get(0).uuid), "offline UUID in world file");
			require(commands.execute(host, "banlist players") == 1, "ban list");
			require(commands.getCompletions(host, "pardon Arc", host.getBlockPos()).contains(guest.getName()),
					"pardon suggestion");
			require(commands.execute(host, "pardon " + recased.getName()) == 1, "pardon command");
			require(players.checkCanJoin(tunnel, recased) == null, "profile pardon");

			require(commands.execute(host, "ban-ip 192.168.10.12 " + reason) == 1, "IP ban command");
			require(message.equals(players.checkCanJoin(lan, guest)), "LAN address and reason");
			require(players.checkCanJoin(tunnel, guest) == null, "tunnel ignores LAN address ban");
			require(commands.execute(host, "banlist ips") == 1, "IP ban list");
			require(commands.execute(host, "pardon-ip 192.168.10.12") == 1, "IP pardon command");
			require(players.checkCanJoin(lan, guest) == null, "IP pardon");
			ArcticLegacy.LOG.info("hostban: PASSED");
		} catch (Exception | AssertionError ex) {
			ArcticLegacy.LOG.error("hostban: FAILED", ex);
		} finally {
			LegacyHooks.onNextFrame(mc::scheduleStop);
		}
	}

	private static GameProfile offline(String name) {
		return new GameProfile(OfflineJoin.offlineId(name), name);
	}

	private static void require(boolean ok, String step) {
		if (!ok) throw new AssertionError(step);
	}
}
