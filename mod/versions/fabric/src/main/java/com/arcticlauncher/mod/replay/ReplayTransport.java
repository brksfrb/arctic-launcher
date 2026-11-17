//#if MC >= 1.13
package com.arcticlauncher.mod.replay;

import io.netty.channel.Channel;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.embedded.EmbeddedChannel;
import net.minecraft.network.Connection;
import net.minecraft.network.protocol.PacketFlow;

/** A connection with Minecraft's protocol handover machinery, but no socket. */
public final class ReplayTransport {
	private ReplayTransport() {}

	public static EmbeddedChannel open(Connection connection) {
		return new EmbeddedChannel(new ChannelInitializer<Channel>() {
			@Override
			protected void initChannel(Channel channel) {
				//#if MC >= 1.20.2
				// setupInbound/OutboundProtocol writes configuration tasks through this
				// pipeline. A bare EmbeddedChannel(connection) silently discards them.
				Connection.configureInMemoryPipeline(channel.pipeline(), PacketFlow.CLIENTBOUND);
				connection.configurePacketHandler(channel.pipeline());
				//#else
				channel.pipeline().addLast("packet_handler", connection);
				//#endif
			}
		});
	}
}
//#endif
