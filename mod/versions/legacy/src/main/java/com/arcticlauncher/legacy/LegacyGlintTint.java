package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.awt.image.BufferedImage;
import java.io.InputStream;
import javax.imageio.ImageIO;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.Identifier;

/**
 * Glint Colorizer (1.8.9 - 1.12.2): the glint is one grey texture the game multiplies by its purple (x0.5, x0.25,
 * x0.8 on items; armor the same or 76% of it). The game's copy is replaced by one made so that product comes out
 * as the chosen color (brightness about 60% of the pick, as the game's purple is), items and armor alike. A resource
 * reload brings the game's own back; it's put in again within a second.
 */
public final class LegacyGlintTint {
	private static final Identifier GLINT = new Identifier("textures/misc/enchanted_item_glint.png");
	private static final long CHECK_MS = 1000;
	/** The game's purple per channel. */
	private static final double[] GAME = {0.5, 0.25, 0.8};
	private static int applied;
	private static Object ours;
	private static long checkedAt;

	private LegacyGlintTint() {}

	/** Each frame (cheap: looks about once a second). */
	public static void tick() {
		ClientConfig c = ArcticClient.config();
		long now = System.currentTimeMillis();
		MinecraftClient mc = MinecraftClient.getInstance();
		if (c == null || mc == null || mc.getTextureManager() == null || now - checkedAt < CHECK_MS) {
			return;
		}
		checkedAt = now;
		int want = c.glintColor & 0xFFFFFF;
		boolean stillOurs = ours != null && mc.getTextureManager().getTexture(GLINT) == ours;
		if (want == 0 && !stillOurs) {
			applied = 0;
			ours = null;
			return;
		}
		if (want != 0 && want == applied && stillOurs) {
			return;
		}
		try (InputStream in = mc.getResourceManager().getResource(GLINT).getInputStream()) {
			BufferedImage original = ImageIO.read(in);
			BufferedImage image = want == 0 ? original : tinted(original, want);
			if (LegacyTextures.register(GLINT, image)) {
				ours = want == 0 ? null : mc.getTextureManager().getTexture(GLINT);
				applied = want;
			}
		} catch (Exception e) {
			ArcticLegacy.LOG.warn("Arctic glint: {}", e.toString());
			applied = want;
		}
	}

	private static BufferedImage tinted(BufferedImage image, int target) {
		int w = image.getWidth();
		int h = image.getHeight();
		double total = 0;
		for (int y = 0; y < h; y++) {
			for (int x = 0; x < w; x++) {
				total += luma(image.getRGB(x, y));
			}
		}
		double mean = Math.max(1e-3, total / (w * h));
		double[] base = new double[3];
		for (int i = 0; i < 3; i++) {
			base[i] = Math.min(1, (target >> (16 - 8 * i) & 0xFF) / 255.0 / GAME[i] * 0.6);
		}
		BufferedImage out = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
		for (int y = 0; y < h; y++) {
			for (int x = 0; x < w; x++) {
				int argb = image.getRGB(x, y);
				double t = luma(argb) / mean * 255.0;
				out.setRGB(x, y, (argb & 0xFF000000) | clamp(t * base[0]) << 16 | clamp(t * base[1]) << 8 | clamp(t * base[2]));
			}
		}
		return out;
	}

	private static double luma(int argb) {
		return 0.299 * (argb >> 16 & 0xFF) + 0.587 * (argb >> 8 & 0xFF) + 0.114 * (argb & 0xFF);
	}

	private static int clamp(double v) {
		return (int) Math.max(0, Math.min(255, Math.round(v)));
	}
}
