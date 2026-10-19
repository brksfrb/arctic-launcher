package com.arcticlauncher.client.net;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;
import io.netty.channel.embedded.EmbeddedChannel;
import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

/** The HTTP tunnel against a scripted proxy, on Netty 4.0 (what 1.8.9 ships). */
class HttpConnectHandlerTest {
	private static final class Game extends ChannelInboundHandlerAdapter {
		boolean active;
		Throwable error;
		final StringBuilder received = new StringBuilder();

		@Override
		public void channelActive(ChannelHandlerContext ctx) {
			active = true;
		}

		@Override
		public void channelRead(ChannelHandlerContext ctx, Object msg) {
			ByteBuf in = (ByteBuf) msg;
			received.append(in.toString(StandardCharsets.ISO_8859_1));
			in.release();
		}

		@Override
		public void exceptionCaught(ChannelHandlerContext ctx, Throwable cause) {
			error = cause;
		}
	}

	private static EmbeddedChannel connect(HttpConnectHandler handler, Game game) {
		EmbeddedChannel ch = new EmbeddedChannel(new ChannelInboundHandlerAdapter());
		ch.pipeline().addFirst(handler, game);
		ch.pipeline().fireChannelActive();
		return ch;
	}

	private static String sent(EmbeddedChannel ch) {
		ByteBuf out = (ByteBuf) ch.readOutbound();
		assertNotNull(out, "nothing sent");
		String text = out.toString(StandardCharsets.UTF_8);
		out.release();
		return text;
	}

	private static ByteBuf reply(String text) {
		return Unpooled.wrappedBuffer(text.getBytes(StandardCharsets.ISO_8859_1));
	}

	@Test
	void asksForTheServerByNameAndLogsIn() {
		Game game = new Game();
		EmbeddedChannel ch = connect(new HttpConnectHandler("play.example.com", 25565, "me", "pw"), game);

		String request = sent(ch);
		assertTrue(request.startsWith("CONNECT play.example.com:25565 HTTP/1.1\r\n"), request);
		// "me:pw" in Base64.
		assertTrue(request.contains("Proxy-Authorization: Basic bWU6cHc=\r\n"), request);
		assertTrue(request.endsWith("\r\n\r\n"));
		assertFalse(game.active, "the game waits until the tunnel is up");

		// The answer arrives in pieces, with the server's first bytes right behind it.
		ch.writeInbound(reply("HTTP/1.1 200 Connection es"));
		assertFalse(game.active);
		ch.writeInbound(reply("tablished\r\nX: y\r\n\r\nHI"));
		assertTrue(game.active);
		assertNull(game.error);
		assertEquals("HI", game.received.toString());
		assertNull(ch.pipeline().get(HttpConnectHandler.class), "the handler steps aside");
		ch.finish();
	}

	@Test
	void noLoginMeansNoAuthorizationHeader() {
		String request = HttpConnectHandler.request("1.2.3.4", 25565, "", "");
		assertFalse(request.contains("Proxy-Authorization"));
		assertTrue(request.startsWith("CONNECT 1.2.3.4:25565 HTTP/1.1"));
		assertTrue(HttpConnectHandler.request("::1", 25565, null, null).startsWith("CONNECT [::1]:25565 "));
	}

	@Test
	void aRefusedLoginIsReportedToTheGame() {
		Game game = new Game();
		EmbeddedChannel ch = connect(new HttpConnectHandler("play.example.com", 25565, "me", "wrong"), game);
		sent(ch);
		ch.writeInbound(reply("HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic\r\n\r\n"));
		assertNotNull(game.error);
		assertTrue(game.error.getMessage().contains("user name or password"), game.error.getMessage());
		assertFalse(ch.isOpen());
	}

	@Test
	void statusLinesAreJudgedByTheirCode() {
		assertNull(HttpConnectHandler.problem("HTTP/1.0 200 OK"));
		assertEquals("the proxy can't reach the server", HttpConnectHandler.problem("HTTP/1.1 502 Bad Gateway"));
		assertEquals("that isn't an HTTP proxy", HttpConnectHandler.problem("\u0005\u0000"));
	}
}
