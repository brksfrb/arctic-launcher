package com.arcticlauncher.client.together;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.together.HostedBanCommands.Action;
import com.arcticlauncher.client.together.HostedBanCommands.Rejected;
import io.netty.channel.local.LocalAddress;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class HostedBanCommandsTest {
	@TempDir
	Path world;

	@Test
	void refusesTunnelAddressThenBansAndDisconnectsOnlyTheNamedProfile() throws Exception {
		Lang.init(null, "en");
		Player host = new Player("Host", new LocalAddress("hostban-test"));
		Player friend = new Player("Friend", new InetSocketAddress("127.0.0.1", 1234));
		Player other = new Player("Other", new InetSocketAddress("127.0.0.1", 1235));
		WorldBanStore store = new WorldBanStore(world);
		Commands commands = new Commands(store, host, friend, other);

		Rejected failure = assertThrows(Rejected.class, () -> commands.run(Action.BAN_IP, "Friend", null, ignored -> {}));
		assertEquals(Lang.t("social.hostban_tunnel_ip"), failure.getMessage());
		assertTrue(store.entries().isEmpty());
		assertNull(friend.kickedWith);
		assertThrows(Rejected.class, () -> commands.run(Action.BAN, "hOsT", null, ignored -> {}));

		assertEquals(1, commands.run(Action.BAN, "fRiEnD", "Test reason", ignored -> {}));
		assertEquals(Lang.t("social.hostban_disconnect", "Test reason"), friend.kickedWith);
		assertNull(other.kickedWith);
		assertNull(host.kickedWith);
		assertEquals(friend.kickedWith, HostedBanCommands.loginMessage(() -> store,
				OfflineJoin.offlineId("FRIEND"), "FRIEND", friend.remote));
		assertNull(HostedBanCommands.loginMessage(() -> store, other.id, other.name, other.remote));
	}

	@Test
	void disconnectsOnlinePlayersAtTheBannedLanAddress() throws Exception {
		Lang.init(null, "en");
		Player one = new Player("One", new InetSocketAddress("192.168.1.6", 1234));
		Player two = new Player("Two", new InetSocketAddress("192.168.1.6", 1235));
		Player three = new Player("Three", new InetSocketAddress("192.168.1.7", 1234));
		WorldBanStore store = new WorldBanStore(world);
		Commands commands = new Commands(store, one, two, three);

		assertEquals(1, commands.run(Action.BAN_IP, "One", null, ignored -> {}));
		assertEquals(Lang.t("social.hostban_disconnect", Lang.t("social.hostban_default_reason")), one.kickedWith);
		assertEquals(one.kickedWith, two.kickedWith);
		assertNull(three.kickedWith);
		assertFalse(store.entries().isEmpty());
	}

	private static final class Player {
		final String name;
		final UUID id;
		final SocketAddress remote;
		String kickedWith;

		Player(String name, SocketAddress remote) {
			this.name = name;
			this.id = OfflineJoin.offlineId(name);
			this.remote = remote;
		}
	}

	private static final class Commands extends HostedBanCommands<Player> {
		private final List<Player> players;

		Commands(WorldBanStore store, Player... players) {
			super(() -> store);
			this.players = Arrays.asList(players);
		}

		@Override protected List<Player> players() { return players; }
		@Override protected String name(Player player) { return player.name; }
		@Override protected UUID id(Player player) { return player.id; }
		@Override protected SocketAddress remote(Player player) { return player.remote; }
		@Override protected Profile resolveProfile(String name) { return null; }
		@Override protected String hostName() { return "Host"; }
		@Override protected void disconnect(Player player, String message) { player.kickedWith = message; }
	}
}
