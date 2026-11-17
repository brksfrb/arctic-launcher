package com.arcticlauncher.client.replay;

import io.netty.channel.Channel;
import io.netty.channel.ChannelHandler;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.embedded.EmbeddedChannel;

/** The channel a replay's connection lives in (old versions, Netty 4.0). */
public final class ReplayChannel {
	/** What the game calls its connection in a real pipeline; Forge inserts its handshake handler before it. */
	public static final String CONNECTION = "packet_handler";

	private ReplayChannel() {}

	/**
	 * A channel with {@code connection} in it under the game's own handler name.
	 * Like a real connection, the handler is in place before the channel becomes
	 * active (so it sees channelActive); an initializer does that, because Netty
	 * 4.0 only runs channelActive for handlers present at construction.
	 */
	public static EmbeddedChannel open(final ChannelHandler connection) {
		return new EmbeddedChannel(new ChannelInitializer<Channel>() {
			@Override
			protected void initChannel(Channel ch) {
				ch.pipeline().addLast(CONNECTION, connection);
			}
		});
	}
}
