package com.arcticlauncher.client.looks;

import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.security.MessageDigest;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ThreadFactory;
import java.util.concurrent.TimeUnit;
import javax.imageio.ImageIO;

/**
 * OptiFine's capes without OptiFine: OptiFine players' capes are public (one PNG per player name on its cape
 * server), so a player with no Arctic cape and none of Mojang's can still show theirs. Names are asked one
 * at a time on a thread of their own, each once (again later when the server couldn't be reached).
 */
final class OptifineCapes {
	private static final String URL = "http://s.optifine.net/capes/";
	/** A name that couldn't be asked (network) is asked again after this long. */
	private static final long RETRY_MS = 10 * 60 * 1000L;
	/** OptiFine's own cape layout (46 × 22, or a multiple), which sits in the corner of Minecraft's 64 × 32. */
	private static final int OWN_W = 46;
	private static final int OWN_H = 22;
	private static final int MC_W = 64;
	private static final int MC_H = 32;
	/** Valid Minecraft names only (the name goes into a URL). */
	private static final int MAX_NAME = 16;
	/** The widest image looked at (the widest cape Arctic takes). */
	private static final int MAX_WIDTH = 1024;

	/** Name → texture hash; "" when that player has no OptiFine cape. */
	private final Map<String, String> found = new ConcurrentHashMap<String, String>();
	private final Set<String> asked = java.util.Collections.newSetFromMap(new ConcurrentHashMap<String, Boolean>());
	private final Looks looks;
	private final ScheduledExecutorService worker = Executors.newSingleThreadScheduledExecutor(new ThreadFactory() {
		@Override
		public Thread newThread(Runnable r) {
			Thread t = new Thread(r, "arctic-optifine-capes");
			t.setDaemon(true);
			return t;
		}
	});

	OptifineCapes(Looks looks) {
		this.looks = looks;
	}

	/** The texture hash of the player's OptiFine cape, once known; null while asking, or when there is none. */
	String hashFor(String name) {
		if (!validName(name)) {
			return null;
		}
		String hash = found.get(name);
		if (hash != null) {
			return hash.isEmpty() ? null : hash;
		}
		if (asked.add(name)) {
			worker.execute(new Runnable() {
				@Override
				public void run() {
					try {
						load(name);
					} catch (Throwable t) {
						looks.survive(t, "optifine cape " + name);
					}
				}
			});
		}
		return null;
	}

	private static boolean validName(String name) {
		if (name == null || name.isEmpty() || name.length() > MAX_NAME) {
			return false;
		}
		for (int i = 0; i < name.length(); i++) {
			char c = name.charAt(i);
			if (!(c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || c == '_')) {
				return false;
			}
		}
		return true;
	}

	private void load(final String name) {
		try {
			byte[] png = layout(Http.getBytes(URL + name + ".png", null));
			if (!Looks.fitsTexture(png, true)) {
				throw new Http.Failed(404, "not a cape");
			}
			String hash = sha1(png);
			looks.adoptCape(hash, png);
			found.put(name, hash);
		} catch (Exception e) {
			if (e instanceof Http.Failed && ((Http.Failed) e).status == 404) {
				// No cape for this player.
				found.put(name, "");
				return;
			}
			looks.log("optifine cape " + name + ": " + e);
			worker.schedule(new Runnable() {
				@Override
				public void run() {
					asked.remove(name);
				}
			}, RETRY_MS, TimeUnit.MILLISECONDS);
		}
	}

	/** OptiFine's 46 × 22 layout moved into the corner of a 64 × 32 texture, as OptiFine does; others as they are. */
	static byte[] layout(byte[] png) throws java.io.IOException {
		// The size is read from the header first: nothing bigger than a cape is ever decoded.
		int[] size = Looks.pngSize(png);
		if (size == null || size[0] > MAX_WIDTH || size[1] > MAX_WIDTH) {
			throw new java.io.IOException("not a cape-sized PNG");
		}
		int scale = size[0] / OWN_W;
		if (scale < 1 || size[0] != OWN_W * scale || size[1] != OWN_H * scale) {
			return png;
		}
		BufferedImage in = ImageIO.read(new ByteArrayInputStream(png));
		if (in == null) {
			throw new java.io.IOException("not a PNG");
		}
		BufferedImage out = new BufferedImage(MC_W * scale, MC_H * scale, BufferedImage.TYPE_INT_ARGB);
		out.getGraphics().drawImage(in, 0, 0, null);
		ByteArrayOutputStream bytes = new ByteArrayOutputStream();
		ImageIO.write(out, "png", bytes);
		return bytes.toByteArray();
	}

	private static String sha1(byte[] data) throws java.security.NoSuchAlgorithmException {
		byte[] digest = MessageDigest.getInstance("SHA-1").digest(data);
		StringBuilder hex = new StringBuilder(digest.length * 2);
		for (byte b : digest) {
			hex.append(Character.forDigit(b >> 4 & 0xF, 16)).append(Character.forDigit(b & 0xF, 16));
		}
		return hex.toString();
	}
}
