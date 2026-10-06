package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.mojang.blaze3d.platform.GLX;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gl.Framebuffer;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL14;

/**
 * Motion blur for 1.8.9 to 1.12.2: the last frame is kept in a framebuffer of its own and drawn over the
 * new one, partly see-through, after the world and before the HUD (so the HUD stays sharp). The result is
 * kept as the next frame's "last frame": out = mix(now, previous, strength), previous = out.
 */
public final class LegacyMotionBlur {
	/** How much of the last frame stays, for strength 1 (Low) to 4 (Max); as on 26.x (mod/tools/motion_blur.py). */
	private static final float[] STRENGTHS = {0.5f, 0.65f, 0.78f, 0.88f};
	private static final int SRC_ALPHA = 770;
	private static final int ONE_MINUS_SRC_ALPHA = 771;
	/** GL_CONSTANT_ALPHA and GL_ONE_MINUS_CONSTANT_ALPHA (OpenGL 1.4; LWJGL 2's GL14 doesn't name them). */
	private static final int CONSTANT_ALPHA = 0x8003;
	private static final int ONE_MINUS_CONSTANT_ALPHA = 0x8004;
	/** The ortho depth range Minecraft's own framebuffer blit uses. */
	private static final double NEAR = 1000.0;
	private static final double FAR = 3000.0;
	private static final float DEPTH = -2000.0f;

	private static Framebuffer previous;

	private LegacyMotionBlur() {}

	/** Right after the world is drawn into the main framebuffer. */
	public static void apply() {
		ClientConfig config = ArcticClient.config();
		if (config == null || !config.motionBlur || !GLX.supportsFbo()) {
			release();
			return;
		}
		Framebuffer main = MinecraftClient.getInstance().getFramebuffer();
		int width = main.textureWidth;
		int height = main.textureHeight;
		if (previous == null || previous.textureWidth != width || previous.textureHeight != height) {
			release();
			previous = new Framebuffer(width, height, false);
			// Start from this frame, not from black.
			copy(main, previous, width, height);
			main.bind(true);
			return;
		}
		int level = Math.max(1, Math.min(STRENGTHS.length, config.motionBlurStrength));
		main.bind(true);
		draw(previous, width, height, STRENGTHS[level - 1]);
		copy(main, previous, width, height);
		main.bind(true);
	}

	/** Motion blur off (or the window is gone): the last frame isn't kept. */
	private static void release() {
		if (previous != null) {
			previous.delete();
			previous = null;
		}
	}

	private static void copy(Framebuffer from, Framebuffer to, int width, int height) {
		to.bind(true);
		draw(from, width, height, 1f);
	}

	/** Draw {@code source} over the bound framebuffer with this opacity. */
	private static void draw(Framebuffer source, int width, int height, float opacity) {
		GlStateManager.matrixMode(GL11.GL_PROJECTION);
		GlStateManager.pushMatrix();
		GlStateManager.loadIdentity();
		GlStateManager.ortho(0.0, width, height, 0.0, NEAR, FAR);
		GlStateManager.matrixMode(GL11.GL_MODELVIEW);
		GlStateManager.pushMatrix();
		GlStateManager.loadIdentity();
		GlStateManager.translate(0.0f, 0.0f, DEPTH);
		GlStateManager.viewport(0, 0, width, height);
		GlStateManager.colorMask(true, true, true, false);
		GlStateManager.disableDepthTest();
		GlStateManager.depthMask(false);
		GlStateManager.disableLighting();
		GlStateManager.disableAlphaTest();
		GlStateManager.enableTexture();
		// The frames' alpha channel isn't kept (it's 0 in a fresh framebuffer), so the opacity is a constant.
		if (opacity < 1f) {
			GlStateManager.enableBlend();
			GL14.glBlendColor(0f, 0f, 0f, opacity);
			GlStateManager.blendFunc(CONSTANT_ALPHA, ONE_MINUS_CONSTANT_ALPHA);
		} else {
			GlStateManager.disableBlend();
		}
		GlStateManager.color(1f, 1f, 1f, 1f);
		GlStateManager.bindTexture(source.colorAttachment);
		GL11.glBegin(GL11.GL_QUADS);
		GL11.glTexCoord2f(0f, 0f);
		GL11.glVertex3f(0f, height, 0f);
		GL11.glTexCoord2f(1f, 0f);
		GL11.glVertex3f(width, height, 0f);
		GL11.glTexCoord2f(1f, 1f);
		GL11.glVertex3f(width, 0f, 0f);
		GL11.glTexCoord2f(0f, 1f);
		GL11.glVertex3f(0f, 0f, 0f);
		GL11.glEnd();
		GlStateManager.bindTexture(0);
		GL14.glBlendColor(0f, 0f, 0f, 0f);
		GlStateManager.blendFunc(SRC_ALPHA, ONE_MINUS_SRC_ALPHA);
		GlStateManager.disableBlend();
		GlStateManager.enableAlphaTest();
		GlStateManager.depthMask(true);
		GlStateManager.enableDepthTest();
		GlStateManager.colorMask(true, true, true, true);
		GlStateManager.matrixMode(GL11.GL_PROJECTION);
		GlStateManager.popMatrix();
		GlStateManager.matrixMode(GL11.GL_MODELVIEW);
		GlStateManager.popMatrix();
	}
}
