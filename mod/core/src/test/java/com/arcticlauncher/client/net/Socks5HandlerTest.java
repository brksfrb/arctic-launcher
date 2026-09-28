package com.arcticlauncher.client.net;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;
import io.netty.channel.embedded.EmbeddedChannel;
import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

/** The handshake against a scripted proxy, on Netty 4.0 (what 1.8.9 ships). */
class Socks5HandlerTest {
	/** Stands in for the game's handler: records what it's told. */
	private static final class Game extends ChannelInboundHandlerAdapter {
		boolean active;
		Throwable error;

		@Override
		public void channelActive(ChannelHandlerContext ctx) {
			active = true;
		}

		@Override
		public void exceptionCaught(ChannelHandlerContext ctx, Throwable cause) {
			error = cause;
		}
	}

	/** Like the game: the handlers go in before the channel is active. */
	private static EmbeddedChannel connect(Socks5Handler socks, Game game) {
		// Netty 4.0 wants a handler up front; this one just passes everything on.
		EmbeddedChannel ch = new EmbeddedChannel(new ChannelInboundHandlerAdapter());
		ch.pipeline().addFirst(socks, game);
		ch.pipeline().fireChannelActive();
		if (game.error != null) {
			throw new AssertionError("activation failed", game.error);
		}
		return ch;
	}

	private static byte[] sent(EmbeddedChannel ch) {
		ByteBuf out = (ByteBuf) ch.readOutbound();
		if (out == null) {
			throw new AssertionError("nothing sent; open " + ch.isOpen() + " active " + ch.isActive() + " pipeline " + ch.pipeline().names());
		}
		byte[] bytes = new byte[out.readableBytes()];
		out.readBytes(bytes);
		out.release();
		return bytes;
	}

	@Test
	void asksTheProxyForTheServerByName() {
		Game game = new Game();
		EmbeddedChannel ch = connect(new Socks5Handler("play.example.com", 25565, null, null), game);

		assertArrayEquals(new byte[] {5, 1, 0}, sent(ch));
		assertFalse(game.active, "the game waits until the tunnel is up");

		ch.writeInbound(Unpooled.wrappedBuffer(new byte[] {5, 0}));
		byte[] name = "play.example.com".getBytes(StandardCharsets.UTF_8);
		byte[] connect = sent(ch);
		assertArrayEquals(new byte[] {5, 1, 0, 3, (byte) name.length}, java.util.Arrays.copyOf(connect, 5));
		assertArrayEquals(name, java.util.Arrays.copyOfRange(connect, 5, 5 + name.length));

		ch.writeInbound(Unpooled.wrappedBuffer(new byte[] {5, 0, 0, 1, 0, 0, 0, 0, 0, 0}));
		assertTrue(game.active);
		assertNull(game.error);
		assertNull(ch.pipeline().get(Socks5Handler.class), "the handler steps aside");
		ch.finish();
	}

	@Test
	void reportsARefusedLoginToTheGame() {
		Game game = new Game();
		EmbeddedChannel ch = connect(new Socks5Handler("play.example.com", 25565, "me", "secret"), game);
		assertArrayEquals(new byte[] {5, 2, 0, 2}, sent(ch));

		ch.writeInbound(Unpooled.wrappedBuffer(new byte[] {5, 2}));
		sent(ch);
		ch.writeInbound(Unpooled.wrappedBuffer(new byte[] {1, 1}));

		assertTrue(game.active, "the game sees the channel so it can show why");
		assertTrue(game.error.getMessage().contains("user name or password"), game.error.getMessage());
		ch.finish();
	}
}
