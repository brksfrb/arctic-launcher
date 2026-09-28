package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.config.ProxyConfig;
import com.arcticlauncher.client.net.ProxyRoutes;
import com.arcticlauncher.client.net.Socks5Handler;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import io.netty.bootstrap.AbstractBootstrap;
import io.netty.bootstrap.Bootstrap;
import io.netty.channel.Channel;
import io.netty.channel.ChannelFuture;
import io.netty.channel.ChannelHandler;
import io.netty.channel.ChannelInitializer;
import java.net.InetAddress;
import java.net.UnknownHostException;
import net.minecraft.network.ClientConnection;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Server connections through the user's SOCKS5 proxy: connect to the proxy
 * instead, with a handshake handler first in the pipeline that asks it for
 * the server by name. The "address" of a proxied server only carries its
 * name (ServerLookupMixin), so nothing was looked up locally.
 */
@Mixin(ClientConnection.class)
abstract class ClientConnectionProxyMixin {
	@WrapOperation(method = "connect(Ljava/net/InetAddress;IZ)Lnet/minecraft/network/ClientConnection;",
			at = @At(value = "INVOKE", target = "Lio/netty/bootstrap/Bootstrap;handler(Lio/netty/channel/ChannelHandler;)Lio/netty/bootstrap/AbstractBootstrap;"))
	private static AbstractBootstrap<?, ?> arctic$proxyHandshake(Bootstrap bootstrap, ChannelHandler handler,
			Operation<AbstractBootstrap<?, ?>> original, @Local(argsOnly = true) InetAddress target, @Local(argsOnly = true) int port) {
		final String host = ProxyRoutes.nameOf(target);
		final ProxyConfig proxy = ProxyRoutes.current();
		if (proxy == null || !ProxyRoutes.routes(host)) {
			return original.call(bootstrap, handler);
		}
		final ChannelHandler game = handler;
		final int serverPort = port;
		return original.call(bootstrap, new ChannelInitializer<Channel>() {
			@Override
			protected void initChannel(Channel channel) {
				channel.pipeline().addLast("arctic_socks5", new Socks5Handler(host, serverPort, proxy.username, proxy.password));
				channel.pipeline().addLast(game);
			}
		});
	}

	@WrapOperation(method = "connect(Ljava/net/InetAddress;IZ)Lnet/minecraft/network/ClientConnection;",
			at = @At(value = "INVOKE", target = "Lio/netty/bootstrap/Bootstrap;connect(Ljava/net/InetAddress;I)Lio/netty/channel/ChannelFuture;"))
	private static ChannelFuture arctic$toProxy(Bootstrap bootstrap, InetAddress address, int port,
			Operation<ChannelFuture> original) throws UnknownHostException {
		ProxyConfig proxy = ProxyRoutes.current();
		if (proxy == null || !ProxyRoutes.routes(ProxyRoutes.nameOf(address))) {
			return original.call(bootstrap, address, port);
		}
		return original.call(bootstrap, InetAddress.getByName(proxy.host()), proxy.port);
	}
}
