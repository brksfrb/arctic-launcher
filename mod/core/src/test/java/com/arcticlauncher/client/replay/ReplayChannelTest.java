package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;
import io.netty.channel.embedded.EmbeddedChannel;
import org.junit.jupiter.api.Test;

/** The replay's channel on Netty 4.0 (what 1.8.9 ships): the game's handler name, as Forge expects. */
class ReplayChannelTest {
	/** Stands in for the game's connection: notes the channel it became active on. */
	private static final class Connection extends ChannelInboundHandlerAdapter {
		io.netty.channel.Channel active;

		@Override
		public void channelActive(ChannelHandlerContext ctx) {
			active = ctx.channel();
		}
	}

	@Test
	void connectionIsNamedPacketHandlerAndSeesChannelActive() {
		Connection conn = new Connection();
		EmbeddedChannel ch = ReplayChannel.open(conn);
		assertSame(conn, ch.pipeline().get("packet_handler"));
		assertSame(ch, conn.active, "the connection must see channelActive (it takes its channel from it)");
	}

	@Test
	void forgeStyleInsertBeforePacketHandlerWorks() {
		// Forge's NetworkDispatcher.insertIntoChannel: pipeline.addBefore("packet_handler", "fml:packet_handler", this).
		EmbeddedChannel ch = ReplayChannel.open(new Connection());
		ch.pipeline().addBefore("packet_handler", "fml:packet_handler", new ChannelInboundHandlerAdapter());
		assertNotNull(ch.pipeline().get("fml:packet_handler"));
		assertEquals("fml:packet_handler", ch.pipeline().names().get(ch.pipeline().names().indexOf("packet_handler") - 1));
		assertTrue(ch.isOpen());
	}
}
