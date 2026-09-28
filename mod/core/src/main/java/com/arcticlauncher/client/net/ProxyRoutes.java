package com.arcticlauncher.client.net;

import com.arcticlauncher.client.config.ProxyConfig;
import java.net.Authenticator;
import java.net.Inet4Address;
import java.net.Inet6Address;
import java.net.InetAddress;
import java.net.PasswordAuthentication;
import java.net.UnknownHostException;
import java.util.regex.Pattern;

/**
 * Which connections go through the user's proxy: every server except this
 * PC and the local network (a proxy can't reach those). Server names stay
 * unresolved so the proxy looks them up, not the local DNS.
 */
public final class ProxyRoutes {
	private static final Pattern IPV4 = Pattern.compile("\\d{1,3}(\\.\\d{1,3}){3}");

	private static volatile ProxyConfig proxy;

	private ProxyRoutes() {}

	/** New settings from Arctic (null = off). */
	public static void set(ProxyConfig p) {
		proxy = p == null ? null : p.copy();
		installLogin(proxy);
	}

	/** The proxy in use, or null. */
	public static ProxyConfig current() {
		return proxy;
	}

	/** Whether a connection to {@code host} goes through the proxy. */
	public static boolean routes(String host) {
		return proxy != null && host != null && !isLocal(host);
	}

	/** No address yet: what {@link #lookup} hands back for servers the proxy will look up. */
	private static final byte[] UNRESOLVED = {0, 0, 0, 0};

	/**
	 * Before 1.17 the game looked server names up itself before connecting.
	 * Through the proxy, this stands in for that lookup: the "address" only
	 * carries the name (see {@link #nameOf}), and no DNS query is made.
	 */
	public static InetAddress lookup(String host) throws UnknownHostException {
		if (!routes(host) || literal(host) != null) {
			return InetAddress.getByName(host);
		}
		return InetAddress.getByAddress(host, UNRESOLVED);
	}

	/** The name (or IP) an address was made from, without a reverse DNS lookup. */
	public static String nameOf(InetAddress address) {
		// toString() is "name/ip", or "/ip" when there's no name.
		String s = address.toString();
		int slash = s.indexOf('/');
		return slash > 0 ? s.substring(0, slash) : address.getHostAddress();
	}

	/** This PC or the local network, judged without any DNS lookup. */
	static boolean isLocal(String host) {
		String h = host.trim().toLowerCase(java.util.Locale.ROOT);
		if (h.equals("localhost") || h.endsWith(".localhost") || h.endsWith(".local") || h.endsWith(".lan")) {
			return true;
		}
		InetAddress ip = literal(h);
		return ip != null && (ip.isLoopbackAddress() || ip.isSiteLocalAddress() || ip.isLinkLocalAddress()
				|| ip.isAnyLocalAddress() || isUniqueLocal(ip));
	}

	/** IPv6 unique local addresses (fc00::/7), the IPv6 private range. */
	private static boolean isUniqueLocal(InetAddress ip) {
		return ip instanceof Inet6Address && (ip.getAddress()[0] & 0xFE) == 0xFC;
	}

	/** The address if {@code host} is an IP literal, else null (never looks names up). */
	static InetAddress literal(String host) {
		String h = host.trim();
		if (h.startsWith("[") && h.endsWith("]")) {
			h = h.substring(1, h.length() - 1);
		}
		if (!IPV4.matcher(h).matches() && h.indexOf(':') < 0) {
			return null;
		}
		try {
			InetAddress ip = InetAddress.getByName(h);
			return ip instanceof Inet4Address || ip instanceof Inet6Address ? ip : null;
		} catch (UnknownHostException | SecurityException e) {
			return null;
		}
	}

	/**
	 * Java's own SOCKS client (Minecraft's web services, Arctic's requests)
	 * asks an {@link Authenticator} for the login; answer only for the proxy.
	 */
	private static void installLogin(final ProxyConfig p) {
		if (p == null || !p.hasLogin()) {
			return;
		}
		Authenticator.setDefault(new Authenticator() {
			@Override
			protected PasswordAuthentication getPasswordAuthentication() {
				String protocol = getRequestingProtocol();
				boolean socks = protocol != null && protocol.toUpperCase(java.util.Locale.ROOT).startsWith("SOCKS");
				if (!socks || !p.host().equalsIgnoreCase(getRequestingHost())) {
					return null;
				}
				return new PasswordAuthentication(p.username, p.password.toCharArray());
			}
		});
	}
}
