package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.config.ProxyConfig;
import com.arcticlauncher.client.net.ProxyRoutes;
import io.netty.bootstrap.AbstractBootstrap;
import io.netty.bootstrap.Bootstrap;
import io.netty.channel.Channel;
import io.netty.channel.ChannelFuture;
import io.netty.channel.ChannelHandler;
import io.netty.channel.ChannelInitializer;
import java.net.InetAddress;
import java.net.UnknownHostException;
import net.minecraft.network.ClientConnection;
import net.minecraft.network.NetworkSide;
import net.minecraft.text.Text;
import com.arcticlauncher.legacy.PendingProxyConnection;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Forge flavour of ClientConnectionProxyMixin (src/main): server connections through the user's SOCKS5
 * proxy. Forge's Mixin has no MixinExtras (wrap operations, local capture), so what {@code connect} is
 * working on (the address, the port, the connection being made) is kept in thread-locals, and the netty
 * calls are redirects.
 */
@Mixin(ClientConnection.class)
abstract class ClientConnectionProxyMixin implements PendingProxyConnection {
	private static final String CONNECT = "connect(Ljava/net/InetAddress;IZ)Lnet/minecraft/network/ClientConnection;";
	@Shadow private Channel channel;
	@Unique private Channel arctic$pendingProxy;
	@Unique private static final ThreadLocal<InetAddress> arctic$target = new ThreadLocal<InetAddress>();
	@Unique private static final ThreadLocal<Integer> arctic$port = new ThreadLocal<Integer>();
	@Unique private static final ThreadLocal<ClientConnection> arctic$connecting = new ThreadLocal<ClientConnection>();

	@Override
	public void arctic$pendingProxy(Channel pending) { arctic$pendingProxy = pending; }

	@Inject(method = "disconnect", at = @At("HEAD"))
	private void arctic$closePendingProxy(Text reason, CallbackInfo ci) {
		// SOCKS deliberately holds channelActive until its handshake completes.
		// Expose the transport only for cleanup, so vanilla can close it and retain
		// the failure reason without sending Minecraft packets into the handshake.
		if (channel == null && arctic$pendingProxy != null) channel = arctic$pendingProxy;
	}

	@Inject(method = CONNECT, at = @At("HEAD"))
	private static void arctic$remember(InetAddress address, int port, boolean useEpoll, CallbackInfoReturnable<ClientConnection> cir) {
		arctic$target.set(address);
		arctic$port.set(port);
	}

	@Redirect(method = CONNECT, at = @At(value = "NEW", target = "net/minecraft/network/ClientConnection"))
	private static ClientConnection arctic$newConnection(NetworkSide side) {
		ClientConnection connection = new ClientConnection(side);
		arctic$connecting.set(connection);
		return connection;
	}

	@Redirect(method = CONNECT,
			at = @At(value = "INVOKE", target = "Lio/netty/bootstrap/Bootstrap;handler(Lio/netty/channel/ChannelHandler;)Lio/netty/bootstrap/AbstractBootstrap;"))
	private static AbstractBootstrap<?, ?> arctic$proxyHandshake(Bootstrap bootstrap, ChannelHandler handler) {
		final String host = ProxyRoutes.nameOf(arctic$target.get());
		final ProxyConfig proxy = ProxyRoutes.current();
		if (proxy == null || !ProxyRoutes.routes(host)) {
			return bootstrap.handler(handler);
		}
		final ChannelHandler game = handler;
		final int serverPort = arctic$port.get();
		final ClientConnection connection = arctic$connecting.get();
		return bootstrap.handler(new ChannelInitializer<Channel>() {
			@Override
			protected void initChannel(Channel channel) {
				((PendingProxyConnection) connection).arctic$pendingProxy(channel);
				channel.pipeline().addLast("arctic_socks5", ProxyRoutes.tunnel(proxy, host, serverPort));
				channel.pipeline().addLast(game);
			}
		});
	}

	@Redirect(method = CONNECT,
			at = @At(value = "INVOKE", target = "Lio/netty/bootstrap/Bootstrap;connect(Ljava/net/InetAddress;I)Lio/netty/channel/ChannelFuture;"))
	private static ChannelFuture arctic$toProxy(Bootstrap bootstrap, InetAddress address, int port) throws UnknownHostException {
		ProxyConfig proxy = ProxyRoutes.current();
		if (proxy == null || !ProxyRoutes.routes(ProxyRoutes.nameOf(address))) {
			return bootstrap.connect(address, port);
		}
		return bootstrap.connect(InetAddress.getByName(proxy.host()), proxy.port);
	}
}
