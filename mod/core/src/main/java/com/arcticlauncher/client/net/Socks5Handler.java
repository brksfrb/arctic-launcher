package com.arcticlauncher.client.net;

import io.netty.buffer.ByteBuf;
import io.netty.channel.ChannelDuplexHandler;
import io.netty.channel.ChannelHandlerContext;
import io.netty.util.concurrent.ScheduledFuture;
import java.io.IOException;
import java.net.Inet4Address;
import java.net.Inet6Address;
import java.net.InetAddress;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.TimeUnit;

/**
 * The client side of a SOCKS5 handshake (RFC 1928, login per RFC 1929),
 * first in a connection's pipeline. The game only sees the channel become
 * active once the proxy has connected it to the server; the server's name
 * is sent to the proxy unresolved, so no DNS lookup happens here.
 */
public final class Socks5Handler extends ChannelDuplexHandler {
	private static final byte VERSION = 5;
	private static final byte NO_AUTH = 0;
	private static final byte USER_PASS = 2;
	private static final byte NO_METHOD = (byte) 0xFF;
	private static final byte LOGIN_VERSION = 1;
	private static final byte CONNECT = 1;
	private static final byte IPV4 = 1;
	private static final byte DOMAIN = 3;
	private static final byte IPV6 = 4;
	private static final int IPV4_LEN = 4;
	private static final int IPV6_LEN = 16;
	private static final int MAX_FIELD = 255;
	private static final long TIMEOUT_SECONDS = 20;

	private enum State { GREETING, LOGIN, CONNECT, DONE }

	private final String host;
	private final int port;
	private final String username;
	private final String password;
	private State state = State.GREETING;
	private ByteBuf pending;
	private ScheduledFuture<?> timeout;

	public Socks5Handler(String host, int port, String username, String password) {
		this.host = host;
		this.port = port;
		this.username = username == null ? "" : username;
		this.password = password == null ? "" : password;
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
		// Connected to the proxy; the game hears about it once the tunnel is up.
		try {
			timeout = ctx.executor().schedule(() -> fail(ctx, "the proxy didn't answer in time"), TIMEOUT_SECONDS, TimeUnit.SECONDS);
		} catch (UnsupportedOperationException e) {
			// Netty 4.0's test channel can't schedule; real connections can.
		}
		boolean login = !username.isEmpty();
		ByteBuf out = ctx.alloc().buffer();
		out.writeByte(VERSION);
		out.writeByte(login ? 2 : 1);
		out.writeByte(NO_AUTH);
		if (login) {
			out.writeByte(USER_PASS);
		}
		ctx.writeAndFlush(out);
	}

	@Override
	public void channelRead(ChannelHandlerContext ctx, Object msg) {
		if (state == State.DONE || !(msg instanceof ByteBuf)) {
			ctx.fireChannelRead(msg);
			return;
		}
		ByteBuf in = (ByteBuf) msg;
		try {
			pending.writeBytes(in);
		} finally {
			in.release();
		}
		try {
			advance(ctx);
		} catch (IOException e) {
			fail(ctx, e.getMessage());
		}
	}

	private void advance(ChannelHandlerContext ctx) throws IOException {
		while (state != State.DONE) {
			switch (state) {
				case GREETING:
					if (pending.readableBytes() < 2) {
						return;
					}
					checkVersion(pending.readByte());
					byte method = pending.readByte();
					if (method == USER_PASS && !username.isEmpty()) {
						sendLogin(ctx);
						state = State.LOGIN;
					} else if (method == NO_AUTH) {
						sendConnect(ctx);
						state = State.CONNECT;
					} else if (method == NO_METHOD || method == USER_PASS) {
						throw new IOException("the proxy needs a user name and password");
					} else {
						throw new IOException("the proxy asked for an unsupported login");
					}
					break;
				case LOGIN:
					if (pending.readableBytes() < 2) {
						return;
					}
					pending.readByte();
					if (pending.readByte() != 0) {
						throw new IOException("the proxy refused the user name or password");
					}
					sendConnect(ctx);
					state = State.CONNECT;
					break;
				case CONNECT:
					int length = replyLength();
					if (length < 0 || pending.readableBytes() < length) {
						return;
					}
					checkVersion(pending.getByte(pending.readerIndex()));
					byte reply = pending.getByte(pending.readerIndex() + 1);
					if (reply != 0) {
						throw new IOException(reason(reply));
					}
					pending.skipBytes(length);
					state = State.DONE;
					open(ctx);
					return;
				default:
					return;
			}
		}
	}

	/** Bytes in the CONNECT reply, or -1 until enough has arrived to tell. */
	private int replyLength() throws IOException {
		if (pending.readableBytes() < 5) {
			return -1;
		}
		byte type = pending.getByte(pending.readerIndex() + 3);
		switch (type) {
			case IPV4:
				return 6 + IPV4_LEN;
			case IPV6:
				return 6 + IPV6_LEN;
			case DOMAIN:
				return 7 + (pending.getByte(pending.readerIndex() + 4) & 0xFF);
			default:
				// A failure reply may carry no usable address; report the code.
				byte reply = pending.getByte(pending.readerIndex() + 1);
				throw new IOException(reply != 0 ? reason(reply) : "the proxy sent a broken reply");
		}
	}

	/** The tunnel is up: step aside and let the game start talking. */
	private void open(ChannelHandlerContext ctx) {
		ByteBuf rest = pending.readableBytes() > 0 ? pending.readSlice(pending.readableBytes()).retain() : null;
		ctx.pipeline().remove(this);
		ctx.fireChannelActive();
		if (rest != null) {
			ctx.fireChannelRead(rest);
		}
	}

	private void sendLogin(ChannelHandlerContext ctx) throws IOException {
		byte[] user = field(username, "user name");
		byte[] pass = field(password, "password");
		ByteBuf out = ctx.alloc().buffer();
		out.writeByte(LOGIN_VERSION);
		out.writeByte(user.length);
		out.writeBytes(user);
		out.writeByte(pass.length);
		out.writeBytes(pass);
		ctx.writeAndFlush(out);
	}

	private void sendConnect(ChannelHandlerContext ctx) throws IOException {
		ByteBuf out = ctx.alloc().buffer();
		out.writeByte(VERSION);
		out.writeByte(CONNECT);
		out.writeByte(0);
		InetAddress literal = ProxyRoutes.literal(host);
		if (literal instanceof Inet4Address) {
			out.writeByte(IPV4);
			out.writeBytes(literal.getAddress());
		} else if (literal instanceof Inet6Address) {
			out.writeByte(IPV6);
			out.writeBytes(literal.getAddress());
		} else {
			byte[] name = field(host, "server name");
			out.writeByte(DOMAIN);
			out.writeByte(name.length);
			out.writeBytes(name);
		}
		out.writeShort(port);
		ctx.writeAndFlush(out);
	}

	private static byte[] field(String s, String what) throws IOException {
		byte[] bytes = s.getBytes(StandardCharsets.UTF_8);
		if (bytes.length > MAX_FIELD) {
			throw new IOException("the " + what + " is too long for SOCKS5");
		}
		return bytes;
	}

	private static void checkVersion(byte version) throws IOException {
		if (version != VERSION && version != LOGIN_VERSION) {
			throw new IOException("that isn't a SOCKS5 proxy");
		}
	}

	private static String reason(byte reply) {
		switch (reply) {
			case 2:
				return "the proxy's rules don't allow this connection";
			case 3:
				return "the proxy can't reach the network";
			case 4:
				return "the proxy can't reach the server";
			case 5:
				return "the server refused the connection (through the proxy)";
			case 6:
				return "the connection timed out at the proxy";
			default:
				return "the proxy couldn't connect (error " + (reply & 0xFF) + ")";
		}
	}

	private void fail(ChannelHandlerContext ctx, String why) {
		if (state == State.DONE || !ctx.channel().isOpen()) {
			return;
		}
		state = State.DONE;
		// Let the game see the channel first, so it shows the reason instead
		// of waiting on a connection it never saw open.
		ctx.fireChannelActive();
		ctx.fireExceptionCaught(new IOException("Proxy: " + why));
		ctx.close();
	}
}
