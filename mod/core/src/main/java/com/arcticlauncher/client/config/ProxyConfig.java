package com.arcticlauncher.client.config;

/**
 * A SOCKS5 proxy for server connections and Arctic's own web requests.
 * Set in the launcher (passed in the session file) or in game.
 */
public final class ProxyConfig {
	public static final int DEFAULT_PORT = 1080;
	private static final int MAX_PORT = 65535;
	private static final int MAX_HOST = 253;

	public boolean enabled;
	public String host = "";
	public int port = DEFAULT_PORT;
	public String username = "";
	public String password = "";

	/** On, with an address that can be used. */
	public boolean usable() {
		return enabled && problem() == null;
	}

	/** Why the address can't be used, or null. */
	public String problem() {
		String h = host == null ? "" : host.trim();
		if (h.isEmpty()) {
			return "Enter the proxy's address";
		}
		if (h.length() > MAX_HOST) {
			return "The address is too long";
		}
		for (int i = 0; i < h.length(); i++) {
			char c = h.charAt(i);
			boolean ok = c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || c == '.' || c == '-' || c == ':';
			if (!ok) {
				return "That isn't a host name or IP address";
			}
		}
		if (port <= 0 || port > MAX_PORT) {
			return "Enter a port from 1 to 65535";
		}
		return null;
	}

	public String host() {
		return host == null ? "" : host.trim();
	}

	public boolean hasLogin() {
		return username != null && !username.isEmpty();
	}

	public ProxyConfig copy() {
		ProxyConfig c = new ProxyConfig();
		c.enabled = enabled;
		c.host = host;
		c.port = port;
		c.username = username;
		c.password = password;
		return c;
	}

	void fillDefaults() {
		if (host == null) {
			host = "";
		}
		if (username == null) {
			username = "";
		}
		if (password == null) {
			password = "";
		}
		if (port <= 0 || port > MAX_PORT) {
			port = DEFAULT_PORT;
		}
	}
}
