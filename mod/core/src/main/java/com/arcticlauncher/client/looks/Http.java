package com.arcticlauncher.client.looks;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;

/** Tiny blocking HTTP client (Java 8, no dependencies). */
public final class Http {
	private static final int CONNECT_MS = 8000;
	private static final int READ_MS = 10000;
	private static final int MAX_BYTES = 4 * 1024 * 1024;

	/** Through the user's SOCKS5 proxy when one is set. */
	private static volatile java.net.Proxy proxy = java.net.Proxy.NO_PROXY;

	private Http() {}

	/** Send requests through this proxy (null = directly). */
	public static void useProxy(com.arcticlauncher.client.config.ProxyConfig p) {
		proxy = p == null ? java.net.Proxy.NO_PROXY
				: new java.net.Proxy(java.net.Proxy.Type.SOCKS, java.net.InetSocketAddress.createUnresolved(p.host(), p.port));
	}

	static byte[] get(String url, String token) throws IOException {
		return request("GET", url, token, null, null);
	}

	/** A download (e.g. a chat screenshot). */
	public static byte[] getBytes(String url, String token) throws IOException {
		return get(url, token);
	}

	/** Upload raw bytes (e.g. a PNG); returns the answer's text. */
	public static String postBytes(String url, String token, byte[] body, String contentType) throws IOException {
		return new String(request("POST", url, token, body, contentType), StandardCharsets.UTF_8);
	}

	public static String getText(String url, String token) throws IOException {
		return new String(get(url, token), StandardCharsets.UTF_8);
	}

	public static String send(String method, String url, String token, String json) throws IOException {
		byte[] body = json == null ? null : json.getBytes(StandardCharsets.UTF_8);
		return new String(request(method, url, token, body, "application/json"), StandardCharsets.UTF_8);
	}

	/** HTTPS trusts the system's certificates too (see {@link OsTrust}). */
	private static void trust(HttpURLConnection c) {
		if (c instanceof javax.net.ssl.HttpsURLConnection) {
			javax.net.ssl.SSLSocketFactory f = OsTrust.factory();
			if (f != null) {
				((javax.net.ssl.HttpsURLConnection) c).setSSLSocketFactory(f);
			}
		}
	}

	/**
	 * Stream a download into a file (for big files like resource packs);
	 * returns its SHA-1 (hex). The file is removed when anything fails.
	 */
	@SuppressWarnings("deprecation")
	public static String downloadTo(String url, java.io.File file, long maxBytes) throws IOException {
		HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection(proxy);
		trust(c);
		java.security.MessageDigest sha1;
		try {
			sha1 = java.security.MessageDigest.getInstance("SHA-1");
		} catch (java.security.NoSuchAlgorithmException e) {
			throw new IOException(e);
		}
		boolean done = false;
		try {
			c.setConnectTimeout(CONNECT_MS);
			c.setReadTimeout(READ_MS);
			c.setRequestProperty("User-Agent", "arctic-client/1");
			int status = c.getResponseCode();
			if (status / 100 != 2) {
				throw new IOException("HTTP " + status);
			}
			try (InputStream in = c.getInputStream(); OutputStream out = new java.io.FileOutputStream(file)) {
				byte[] buf = new byte[65536];
				long total = 0;
				int n;
				while ((n = in.read(buf)) != -1) {
					total += n;
					if (total > maxBytes) {
						throw new IOException("the file is too big");
					}
					sha1.update(buf, 0, n);
					out.write(buf, 0, n);
				}
			}
			done = true;
		} finally {
			c.disconnect();
			if (!done) {
				file.delete();
			}
		}
		StringBuilder hex = new StringBuilder();
		for (byte b : sha1.digest()) {
			hex.append(String.format("%02x", b));
		}
		return hex.toString();
	}

	// new URL(String) is deprecated on new Javas but is what Java 8 has.
	@SuppressWarnings("deprecation")
	private static byte[] request(String method, String url, String token, byte[] body, String contentType)
			throws IOException {
		URL target = new URL(url);
		// This PC (the launcher's bridge) is never reached through the proxy.
		java.net.Proxy route = "127.0.0.1".equals(target.getHost()) ? java.net.Proxy.NO_PROXY : proxy;
		HttpURLConnection c = (HttpURLConnection) target.openConnection(route);
		trust(c);
		try {
			c.setRequestMethod(method);
			c.setConnectTimeout(CONNECT_MS);
			c.setReadTimeout(READ_MS);
			c.setRequestProperty("User-Agent", "arctic-client/1");
			if (token != null) {
				c.setRequestProperty("Authorization", "Bearer " + token);
			}
			if (body != null) {
				c.setDoOutput(true);
				c.setRequestProperty("Content-Type", contentType);
				OutputStream out = c.getOutputStream();
				try {
					out.write(body);
				} finally {
					out.close();
				}
			}
			int status = c.getResponseCode();
			if (status / 100 != 2) {
				throw new IOException(errorOf(c, status));
			}
			return read(c.getInputStream());
		} finally {
			c.disconnect();
		}
	}

	/** The server's own words ({@code {"error": "..."}}), or the status. */
	private static String errorOf(HttpURLConnection c, int status) {
		try {
			InputStream err = c.getErrorStream();
			if (err != null) {
				String text = new String(read(err), StandardCharsets.UTF_8);
				com.google.gson.JsonObject o = new com.google.gson.Gson().fromJson(text, com.google.gson.JsonObject.class);
				if (o != null && o.has("error")) {
					return o.get("error").getAsString();
				}
			}
		} catch (Exception ignored) {
			// Fall back to the status.
		}
		return "HTTP " + status;
	}

	private static byte[] read(InputStream in) throws IOException {
		try {
			ByteArrayOutputStream out = new ByteArrayOutputStream();
			byte[] buf = new byte[8192];
			int n;
			while ((n = in.read(buf)) != -1) {
				if (out.size() + n > MAX_BYTES) {
					throw new IOException("response too large");
				}
				out.write(buf, 0, n);
			}
			return out.toByteArray();
		} finally {
			in.close();
		}
	}
}
