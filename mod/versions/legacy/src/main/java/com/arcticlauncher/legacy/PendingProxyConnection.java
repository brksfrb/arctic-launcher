package com.arcticlauncher.legacy;

import io.netty.channel.Channel;

/** Keep the transport available for cancellation before SOCKS activates Minecraft. */
public interface PendingProxyConnection {
	void arctic$pendingProxy(Channel channel);
}
