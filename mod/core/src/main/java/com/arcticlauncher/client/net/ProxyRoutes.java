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

	/** The first handler of a server connection's pipeline: it opens the tunnel through the proxy. */
	public static io.netty.channel.ChannelHandler tunnel(ProxyConfig proxy, String host, int port) {
		return proxy.isHttp() ? new HttpConnectHandler(host, port, proxy.username, proxy.password)
				: new Socks5Handler(host, port, proxy.username, proxy.password);
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

	private static final java.util.Map<String, String[]> SRV = new java.util.concurrent.ConcurrentHashMap<String, String[]>();
	private static final String[] NO_SRV = new String[0];

	/**
	 * The real (host, port) a server name points to by its {@code _minecraft._tcp} SRV record, or null
	 * when it has none. Many servers (everything behind Cloudflare) only answer on that host, never on
	 * the name's own address. SOCKS and HTTP proxies can't carry DNS, so the record is asked of a DNS-over-HTTPS
	 * service through the proxy itself: nothing is looked up locally, and the lookup never shows your IP.
	 */
	public static String[] srv(String host) {
		if (host == null || literal(host) != null) {
			return null;
		}
		String key = host.toLowerCase(java.util.Locale.ROOT);
		String[] cached = SRV.get(key);
		if (cached == null) {
			long started = System.nanoTime();
			cached = lookupSrv(key);
			SRV.put(key, cached);
			note("proxy: looked up " + key + "'s server record in " + (System.nanoTime() - started) / 1_000_000L + " ms"
					+ (cached == NO_SRV ? " (none)" : " (" + cached[0] + ":" + cached[1] + ")"));
		}
		return cached == NO_SRV ? null : cached.clone();
	}

	/** A line in the game's log: where the time of a join through the proxy goes. */
	static void note(String line) {
		try {
			com.arcticlauncher.client.ArcticClient.platform().log(false, line);
		} catch (RuntimeException e) {
			// No platform yet (tests): nothing to log to.
		}
	}

	private static String[] lookupSrv(String host) {
		try {
			byte[] body = com.arcticlauncher.client.looks.Http.getBytes(
					"https://dns.google/resolve?type=SRV&name=_minecraft._tcp." + host, null);
			com.google.gson.JsonObject answer = new com.google.gson.Gson()
					.fromJson(new String(body, java.nio.charset.StandardCharsets.UTF_8), com.google.gson.JsonObject.class);
			com.google.gson.JsonArray records = answer == null ? null : answer.getAsJsonArray("Answer");
			String[] best = null;
			int bestPriority = Integer.MAX_VALUE;
			if (records != null) {
				for (com.google.gson.JsonElement e : records) {
					com.google.gson.JsonObject record = e.getAsJsonObject();
					if (!record.has("type") || record.get("type").getAsInt() != 33) {
						continue;
					}
					// "priority weight port target."
					String[] parts = record.get("data").getAsString().trim().split("\\s+");
					if (parts.length < 4) {
						continue;
					}
					int priority = Integer.parseInt(parts[0]);
					String target = parts[3].endsWith(".") ? parts[3].substring(0, parts[3].length() - 1) : parts[3];
					if (priority < bestPriority && !target.isEmpty()) {
						bestPriority = priority;
						best = new String[] {target, parts[2]};
					}
				}
			}
			return best == null ? NO_SRV : best;
		} catch (Exception e) {
			// No record found, or the lookup failed: the name's own address, as without SRV.
			return NO_SRV;
		}
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
				boolean httpProxy = p.isHttp() && getRequestorType() == RequestorType.PROXY;
				if (!(p.isHttp() ? httpProxy : socks) || !p.host().equalsIgnoreCase(getRequestingHost())) {
					return null;
				}
				return new PasswordAuthentication(p.username, p.password.toCharArray());
			}
		});
	}
}
