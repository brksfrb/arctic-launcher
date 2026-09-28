package com.arcticlauncher.mod.mixin;

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
import java.net.InetSocketAddress;
import java.net.UnknownHostException;
import net.minecraft.network.Connection;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Server connections through the user's SOCKS5 proxy: connect to the proxy
 * instead, with a handshake handler first in the pipeline that asks it for
 * the server (by name, so the proxy resolves it).
 */
@Mixin(Connection.class)
abstract class ConnectionProxyMixin {
	//#if MC >= 1.17
	@WrapOperation(
			//#if MC >= 1.20
			method = "connect",
			//#else
			method = "connectToServer",
			//#endif
			at = @At(value = "INVOKE", target = "Lio/netty/bootstrap/Bootstrap;handler(Lio/netty/channel/ChannelHandler;)Lio/netty/bootstrap/AbstractBootstrap;"))
	private static AbstractBootstrap<?, ?> arctic$proxyHandshake(Bootstrap bootstrap, ChannelHandler handler,
			Operation<AbstractBootstrap<?, ?>> original, @Local(argsOnly = true) InetSocketAddress target) {
		return arctic$handshake(bootstrap, handler, original, target.getHostString(), target.getPort());
	}

	@WrapOperation(
			//#if MC >= 1.20
			method = "connect",
			//#else
			method = "connectToServer",
			//#endif
			at = @At(value = "INVOKE", target = "Lio/netty/bootstrap/Bootstrap;connect(Ljava/net/InetAddress;I)Lio/netty/channel/ChannelFuture;"))
	private static ChannelFuture arctic$toProxy(Bootstrap bootstrap, InetAddress address, int port,
			Operation<ChannelFuture> original, @Local(argsOnly = true) InetSocketAddress target) throws UnknownHostException {
		return arctic$connect(bootstrap, address, port, original, target.getHostString());
	}
	//#else
	// Before 1.17 the game passed an address it had looked up itself; for proxied
	// servers that "address" only carries the name (ServerLookupMixin).
	@WrapOperation(method = "connectToServer",
			at = @At(value = "INVOKE", target = "Lio/netty/bootstrap/Bootstrap;handler(Lio/netty/channel/ChannelHandler;)Lio/netty/bootstrap/AbstractBootstrap;"))
	private static AbstractBootstrap<?, ?> arctic$proxyHandshake(Bootstrap bootstrap, ChannelHandler handler,
			Operation<AbstractBootstrap<?, ?>> original, @Local(argsOnly = true) InetAddress target, @Local(argsOnly = true) int port) {
		return arctic$handshake(bootstrap, handler, original, ProxyRoutes.nameOf(target), port);
	}

	@WrapOperation(method = "connectToServer",
			at = @At(value = "INVOKE", target = "Lio/netty/bootstrap/Bootstrap;connect(Ljava/net/InetAddress;I)Lio/netty/channel/ChannelFuture;"))
	private static ChannelFuture arctic$toProxy(Bootstrap bootstrap, InetAddress address, int port,
			Operation<ChannelFuture> original) throws UnknownHostException {
		return arctic$connect(bootstrap, address, port, original, ProxyRoutes.nameOf(address));
	}
	//#endif

	private static AbstractBootstrap<?, ?> arctic$handshake(Bootstrap bootstrap, ChannelHandler handler,
			Operation<AbstractBootstrap<?, ?>> original, final String host, final int port) {
		final ProxyConfig proxy = ProxyRoutes.current();
		if (proxy == null || !ProxyRoutes.routes(host)) {
			return original.call(bootstrap, handler);
		}
		final ChannelHandler game = handler;
		return original.call(bootstrap, new ChannelInitializer<Channel>() {
			@Override
			protected void initChannel(Channel channel) {
				channel.pipeline().addLast("arctic_socks5", new Socks5Handler(host, port, proxy.username, proxy.password));
				channel.pipeline().addLast(game);
			}
		});
	}

	private static ChannelFuture arctic$connect(Bootstrap bootstrap, InetAddress address, int port,
			Operation<ChannelFuture> original, String host) throws UnknownHostException {
		ProxyConfig proxy = ProxyRoutes.current();
		if (proxy == null || !ProxyRoutes.routes(host)) {
			return original.call(bootstrap, address, port);
		}
		return original.call(bootstrap, InetAddress.getByName(proxy.host()), proxy.port);
	}
}
