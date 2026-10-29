package com.arcticlauncher.mod;

//#if MC >= 1.13
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.mojang.blaze3d.platform.NativeImage;
import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import javax.imageio.ImageIO;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.client.renderer.texture.TextureManager;

/**
 * Glint Colorizer (1.13 and newer): the shimmer on enchanted items and armor is one texture. From 1.15 it is
 * already purple, so the game's copy is replaced by one in the player's color (the same pattern, its average the
 * chosen color). In 1.13 and 1.14 it is grey and the game multiplies it by its purple (x0.5, x0.25, x0.8 on items,
 * 76% of that on armor): the copy is made so that product comes out as the chosen color.
 * A resource reload brings the game's own back; it's put in again within a second.
 */
public final class GlintTint {
	private static final net.minecraft.resources.Identifier GLINT = Compat.id("minecraft", "textures/misc/enchanted_item_glint.png");
	private static final long CHECK_MS = 1000;
	/** The color in the game's copy now (0: the pack's own). */
	private static int applied;
	/** What was put in, to notice a reload replacing it. */
	private static Object ours;
	private static long checkedAt;

	private GlintTint() {}

	/** Each game tick (cheap: looks about once a second). */
	public static void tick() {
		ClientConfig c = ArcticClient.config();
		long now = System.currentTimeMillis();
		if (c == null || now - checkedAt < CHECK_MS) {
			return;
		}
		checkedAt = now;
		TextureManager textures = Minecraft.getInstance().getTextureManager();
		if (textures == null) {
			return;
		}
		int want = c.glintColor & 0xFFFFFF;
		boolean stillOurs = ours != null && textures.getTexture(GLINT) == ours;
		if (want == 0 && !stillOurs) {
			applied = 0;
			ours = null;
			return;
		}
		if (want != 0 && want == applied && stillOurs) {
			return;
		}
		try {
			byte[] png = original();
			NativeImage image = NativeImage.read(new ByteArrayInputStream(want == 0 ? png : tinted(png, want)));
			DynamicTexture texture = Compat.texture("Arctic glint", image);
			textures.register(GLINT, texture);
			ours = want == 0 ? null : texture;
			applied = want;
		} catch (Exception e) {
			ArcticMod.LOG.warn("Arctic glint: {}", e.toString());
			applied = want;
		}
	}

	/** The pack's glint picture, as PNG bytes. */
	private static byte[] original() throws java.io.IOException {
		//#if MC >= 1.19
		try (InputStream in = Minecraft.getInstance().getResourceManager().open(GLINT)) {
		//#else
		try (InputStream in = Minecraft.getInstance().getResourceManager().getResource(GLINT).getInputStream()) {
		//#endif
			ByteArrayOutputStream out = new ByteArrayOutputStream();
			byte[] buffer = new byte[8192];
			for (int n; (n = in.read(buffer)) > 0;) {
				out.write(buffer, 0, n);
			}
			return out.toByteArray();
		}
	}

	/** The same pattern with the average pixel the target color (brighter pixels go up from it, darker ones down). */
	private static byte[] tinted(byte[] png, int target) throws java.io.IOException {
		BufferedImage image = ImageIO.read(new ByteArrayInputStream(png));
		int w = image.getWidth();
		int h = image.getHeight();
		double total = 0;
		for (int y = 0; y < h; y++) {
			for (int x = 0; x < w; x++) {
				total += luma(image.getRGB(x, y));
			}
		}
		double mean = Math.max(1e-3, total / (w * h));
		int tr = target >> 16 & 0xFF;
		int tg = target >> 8 & 0xFF;
		int tb = target & 0xFF;
		//#if MC < 1.15
		// The game multiplies by its purple: divide it out (brightness kept at about 60% of the pick, as the game's purple is).
		double[] factor = {
				Math.min(1, tr / 255.0 / 0.5 * 0.6),
				Math.min(1, tg / 255.0 / 0.25 * 0.6),
				Math.min(1, tb / 255.0 / 0.8 * 0.6)};
		tr = 255;
		tg = 255;
		tb = 255;
		//#endif
		BufferedImage out = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
		for (int y = 0; y < h; y++) {
			for (int x = 0; x < w; x++) {
				int argb = image.getRGB(x, y);
				double t = luma(argb) / mean;
				//#if MC < 1.15
				out.setRGB(x, y, (argb & 0xFF000000) | clamp(tr * t * factor[0]) << 16 | clamp(tg * t * factor[1]) << 8 | clamp(tb * t * factor[2]));
				//#else
				out.setRGB(x, y, (argb & 0xFF000000) | clamp(tr * t) << 16 | clamp(tg * t) << 8 | clamp(tb * t));
				//#endif
			}
		}
		ByteArrayOutputStream bytes = new ByteArrayOutputStream();
		ImageIO.write(out, "png", bytes);
		return bytes.toByteArray();
	}

	private static double luma(int argb) {
		return 0.299 * (argb >> 16 & 0xFF) + 0.587 * (argb >> 8 & 0xFF) + 0.114 * (argb & 0xFF);
	}

	private static int clamp(double v) {
		return (int) Math.max(0, Math.min(255, Math.round(v)));
	}
}
//#endif
