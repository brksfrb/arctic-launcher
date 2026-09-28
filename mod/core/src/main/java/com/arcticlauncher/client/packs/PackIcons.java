package com.arcticlauncher.client.packs;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.security.MessageDigest;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.looks.Http;
import com.google.gson.Gson;
import com.google.gson.JsonObject;

/**
 * Pack icons as textures: Modrinth's (converted to PNG by the launcher; many
 * are WebP) and your packs' own pack.png. Loaded in the background; until
 * then {@link #key} returns null and the list shows a placeholder.
 */
public final class PackIcons {
	private static final int MAX_ICON_BYTES = 512 * 1024;
	private static final String PACK_PNG = "pack.png";

	private final Platform platform;
	private final String bridgeUrl;
	private final String secret;
	private final Gson gson = new Gson();
	/** Source → texture name ("" = failed, don't retry). */
	private final Map<String, String> names = new ConcurrentHashMap<String, String>();
	private final ExecutorService worker = Executors.newFixedThreadPool(2, r -> {
		Thread t = new Thread(r, "arctic-icons");
		t.setDaemon(true);
		return t;
	});

	public PackIcons(Platform platform, int bridgePort, String bridgeSecret) {
		this.platform = platform;
		this.bridgeUrl = bridgePort > 0 && bridgeSecret != null ? "http://127.0.0.1:" + bridgePort : null;
		this.secret = bridgeSecret;
	}

	/** A Modrinth icon's texture key once loaded, else null (starts loading). */
	public String remote(final String url) {
		if (url == null || url.isEmpty() || bridgeUrl == null) {
			return null;
		}
		return lookup(url, () -> {
			JsonObject body = new JsonObject();
			body.addProperty("url", url);
			JsonObject reply = gson.fromJson(Http.send("POST", bridgeUrl + "/v1/icon", secret, body.toString()), JsonObject.class);
			return java.util.Base64.getDecoder().decode(reply.get("png").getAsString());
		});
	}

	/** A pack's own pack.png (zip or folder) once loaded, else null. */
	public String local(final File pack) {
		if (pack == null || !pack.exists()) {
			return null;
		}
		return lookup(pack.getAbsolutePath() + "@" + pack.lastModified(), () -> readPackPng(pack));
	}

	private interface Source {
		byte[] load() throws Exception;
	}

	private String lookup(final String source, final Source load) {
		String name = names.get(source);
		if (name != null) {
			return name.isEmpty() || !ArcticClient.looks().isReady(name) ? null : "look:" + name;
		}
		final String texture = "icon-" + sha1(source);
		names.put(source, texture);
		worker.execute(() -> {
			try {
				byte[] png = load.load();
				if (png == null || png.length == 0) {
					names.put(source, "");
					return;
				}
				platform.registerTexture(texture, png, false);
			} catch (Exception e) {
				names.put(source, "");
			}
		});
		return null;
	}

	private static byte[] readPackPng(File pack) throws IOException {
		if (pack.isDirectory()) {
			File png = new File(pack, PACK_PNG);
			return png.isFile() && png.length() <= MAX_ICON_BYTES ? Files.readAllBytes(png.toPath()) : null;
		}
		try (ZipFile zip = new ZipFile(pack)) {
			ZipEntry entry = zip.getEntry(PACK_PNG);
			if (entry == null || entry.getSize() > MAX_ICON_BYTES) {
				return null;
			}
			try (InputStream in = zip.getInputStream(entry)) {
				ByteArrayOutputStream out = new ByteArrayOutputStream();
				byte[] buf = new byte[8192];
				int n;
				while ((n = in.read(buf)) > 0 && out.size() <= MAX_ICON_BYTES) {
					out.write(buf, 0, n);
				}
				return out.toByteArray();
			}
		}
	}

	private static String sha1(String s) {
		try {
			byte[] d = MessageDigest.getInstance("SHA-1").digest(s.getBytes(StandardCharsets.UTF_8));
			StringBuilder out = new StringBuilder();
			for (int i = 0; i < 10; i++) {
				out.append(String.format("%02x", d[i]));
			}
			return out.toString();
		} catch (java.security.NoSuchAlgorithmException e) {
			return Integer.toHexString(s.hashCode());
		}
	}
}
