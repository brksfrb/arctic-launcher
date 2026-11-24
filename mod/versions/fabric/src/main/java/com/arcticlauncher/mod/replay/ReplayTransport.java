//#if MC >= 1.13
package com.arcticlauncher.mod.replay;

import io.netty.channel.Channel;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;
import io.netty.channel.ChannelPipeline;
import java.util.function.UnaryOperator;
import java.nio.ByteBuffer;
import net.minecraft.network.protocol.Packet;
import io.netty.channel.embedded.EmbeddedChannel;
import net.minecraft.network.Connection;
import net.minecraft.network.protocol.PacketFlow;

/** A connection with Minecraft's protocol handover machinery, but no socket. */
public final class ReplayTransport {
	private ReplayTransport() {}

	public static EmbeddedChannel open(Connection connection, UnaryOperator<Packet<?>> filter) {
		return new EmbeddedChannel(new ChannelInitializer<Channel>() {
			@Override
			protected void initChannel(Channel channel) {
				ChannelPipeline pipeline = channel.pipeline();
				//#if MC >= 1.20.5
				Connection.configureInMemoryPipeline(pipeline, PacketFlow.CLIENTBOUND);
				connection.configurePacketHandler(pipeline);
				pipeline.addAfter("splitter", "arctic:replay_ingress", new ChannelInboundHandlerAdapter());
				//#elif MC >= 1.20.2
				Connection.setInitialProtocolAttributes(channel);
				Connection.configureSerialization(pipeline, PacketFlow.CLIENTBOUND, null);
				connection.configurePacketHandler(pipeline);
				pipeline.addAfter("splitter", "arctic:replay_ingress", new ChannelInboundHandlerAdapter());
				//#elif MC >= 1.19.4
				Connection.configureSerialization(pipeline, PacketFlow.CLIENTBOUND);
				pipeline.addAfter("splitter", "arctic:replay_ingress", new ChannelInboundHandlerAdapter());
				pipeline.addLast("packet_handler", connection);
				//#else
				pipeline.addLast("packet_handler", connection);
				//#endif
				pipeline.addBefore("packet_handler", "arctic:replay_filter", new ChannelInboundHandlerAdapter() {
					@Override
					public void channelRead(ChannelHandlerContext ctx, Object message) {
						Object packet = message instanceof Packet ? filter.apply((Packet<?>) message) : message;
						if (packet != null) ctx.fireChannelRead(packet);
						else io.netty.util.ReferenceCountUtil.release(message);
					}

					@Override
					public void exceptionCaught(ChannelHandlerContext ctx, Throwable error) {
						// Report malformed replay packets to the viewer without sending a network disconnect.
						ctx.pipeline().lastContext().fireExceptionCaught(error);
					}
				});
			}
		});
	}
	//#if MC >= 1.19.4
	static void receive(EmbeddedChannel channel, ByteBuffer bytes) {
		// A recording stores complete packet bodies, so enter after the frame splitter.
		ChannelHandlerContext ingress = channel.pipeline().context("arctic:replay_ingress");
		if (ingress == null) throw new IllegalStateException("Replay channel is closed");
		ingress.fireChannelRead(io.netty.buffer.Unpooled.wrappedBuffer(bytes.duplicate())).fireChannelReadComplete();
		channel.checkException();
	}
	//#endif

}
//#endif
