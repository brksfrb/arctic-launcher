package com.arcticlauncher.client.looks;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;

/** Tiny blocking HTTP client (Java 8, no dependencies). */
final class Http {
	private static final int CONNECT_MS = 8000;
	private static final int READ_MS = 10000;
	private static final int MAX_BYTES = 4 * 1024 * 1024;

	private Http() {}

	static byte[] get(String url, String token) throws IOException {
		return request("GET", url, token, null);
	}

	static String getText(String url, String token) throws IOException {
		return new String(get(url, token), StandardCharsets.UTF_8);
	}

	static String send(String method, String url, String token, String json) throws IOException {
		byte[] body = json == null ? null : json.getBytes(StandardCharsets.UTF_8);
		return new String(request(method, url, token, body), StandardCharsets.UTF_8);
	}

	// new URL(String) is deprecated on new Javas but is what Java 8 has.
	@SuppressWarnings("deprecation")
	private static byte[] request(String method, String url, String token, byte[] body) throws IOException {
		HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
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
				c.setRequestProperty("Content-Type", "application/json");
				OutputStream out = c.getOutputStream();
				try {
					out.write(body);
				} finally {
					out.close();
				}
			}
			int status = c.getResponseCode();
			if (status / 100 != 2) {
				throw new IOException("HTTP " + status);
			}
			return read(c.getInputStream());
		} finally {
			c.disconnect();
		}
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
