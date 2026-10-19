package com.arcticlauncher.client.net;

import io.netty.buffer.ByteBuf;
import io.netty.channel.ChannelDuplexHandler;
import io.netty.channel.ChannelHandlerContext;
import io.netty.util.concurrent.ScheduledFuture;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.concurrent.TimeUnit;

/**
 * The client side of an HTTP proxy tunnel ({@code CONNECT host:port}, login as Basic), first in a
 * connection's pipeline. Like {@link Socks5Handler}: the game only sees the channel become active
 * once the proxy has connected it to the server, and the server's name goes to the proxy unresolved.
 */
public final class HttpConnectHandler extends ChannelDuplexHandler {
	private static final long TIMEOUT_SECONDS = 20;
	/** A proxy's answer is a short header; more than this isn't one. */
	private static final int MAX_HEADER = 8192;
	private static final byte[] END = {'\r', '\n', '\r', '\n'};

	private final String host;
	private final int port;
	private final String username;
	private final String password;
	private boolean done;
	private ByteBuf pending;
	private ScheduledFuture<?> timeout;

	public HttpConnectHandler(String host, int port, String username, String password) {
		this.host = host;
		this.port = port;
		this.username = username == null ? "" : username;
		this.password = password == null ? "" : password;
	}

	/** The CONNECT request for a target, with a Basic login when there is one. */
	public static String request(String host, int port, String username, String password) {
		String target = (host.indexOf(':') >= 0 && !host.startsWith("[") ? "[" + host + "]" : host) + ":" + port;
		StringBuilder sb = new StringBuilder("CONNECT ").append(target).append(" HTTP/1.1\r\nHost: ").append(target).append("\r\n");
		if (username != null && !username.isEmpty()) {
			String login = username + ":" + (password == null ? "" : password);
			sb.append("Proxy-Authorization: Basic ")
					.append(Base64.getEncoder().encodeToString(login.getBytes(StandardCharsets.UTF_8))).append("\r\n");
		}
		return sb.append("Proxy-Connection: keep-alive\r\n\r\n").toString();
	}

	/** Why a status line (the proxy's first line) isn't a success, or null when it is. */
	public static String problem(String statusLine) {
		String[] parts = statusLine.trim().split(" ", 3);
		if (parts.length < 2 || !parts[0].startsWith("HTTP/")) {
			return "that isn't an HTTP proxy";
		}
		switch (parts[1]) {
			case "200":
				return null;
			case "407":
				return "the proxy refused the user name or password";
			case "403":
				return "the proxy's rules don't allow this connection";
			case "502":
			case "503":
			case "504":
				return "the proxy can't reach the server";
			default:
				return "the proxy answered " + parts[1] + (parts.length > 2 ? " " + parts[2] : "");
		}
	}

	@Override
	public void handlerAdded(ChannelHandlerContext ctx) {
		pending = ctx.alloc().buffer();
	}

	@Override
	public void handlerRemoved(ChannelHandlerContext ctx) {
		if (timeout != null) {
			timeout.cancel(false);
		}
		if (pending != null) {
			pending.release();
			pending = null;
		}
	}

	@Override
	public void channelActive(ChannelHandlerContext ctx) {
		try {
			timeout = ctx.executor().schedule(() -> fail(ctx, "the proxy didn't answer in time"), TIMEOUT_SECONDS, TimeUnit.SECONDS);
		} catch (UnsupportedOperationException e) {
			// Netty 4.0's test channel can't schedule; real connections can.
		}
		ByteBuf out = ctx.alloc().buffer();
		out.writeBytes(request(host, port, username, password).getBytes(StandardCharsets.UTF_8));
		ctx.writeAndFlush(out);
	}

	@Override
	public void channelRead(ChannelHandlerContext ctx, Object msg) {
		if (done || !(msg instanceof ByteBuf)) {
			ctx.fireChannelRead(msg);
			return;
		}
		ByteBuf in = (ByteBuf) msg;
		try {
			pending.writeBytes(in);
		} finally {
			in.release();
		}
		int end = endOfHeader();
		if (end < 0) {
			if (pending.readableBytes() > MAX_HEADER) {
				fail(ctx, "that isn't an HTTP proxy");
			}
			return;
		}
		int lineEnd = pending.indexOf(pending.readerIndex(), pending.writerIndex(), (byte) '\r');
		String status = pending.toString(pending.readerIndex(), lineEnd - pending.readerIndex(), StandardCharsets.ISO_8859_1);
		String problem = problem(status);
		if (problem != null) {
			fail(ctx, problem);
			return;
		}
		pending.skipBytes(end - pending.readerIndex());
		done = true;
		ByteBuf rest = pending.readableBytes() > 0 ? pending.readSlice(pending.readableBytes()).retain() : null;
		ctx.pipeline().remove(this);
		ctx.fireChannelActive();
		if (rest != null) {
			ctx.fireChannelRead(rest);
		}
	}

	/** Index just past the blank line that ends the header, or -1 while it hasn't all arrived. */
	private int endOfHeader() {
		int start = pending.readerIndex();
		int last = pending.writerIndex() - END.length;
		for (int i = start; i <= last; i++) {
			boolean match = true;
			for (int k = 0; k < END.length; k++) {
				if (pending.getByte(i + k) != END[k]) {
					match = false;
					break;
				}
			}
			if (match) {
				return i + END.length;
			}
		}
		return -1;
	}

	private void fail(ChannelHandlerContext ctx, String why) {
		if (done || !ctx.channel().isOpen()) {
			return;
		}
		done = true;
		// Let the game see the channel first, so it shows the reason instead
		// of waiting on a connection it never saw open.
		ctx.fireChannelActive();
		ctx.fireExceptionCaught(new IOException("Proxy: " + why));
		ctx.close();
	}
}
