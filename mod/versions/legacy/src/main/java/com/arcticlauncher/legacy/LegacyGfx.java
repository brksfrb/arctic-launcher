package com.arcticlauncher.legacy;

import com.arcticlauncher.client.gfx.Gfx;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawableHelper;
import net.minecraft.client.render.DiffuseLighting;
import net.minecraft.client.util.Window;
import net.minecraft.item.ItemStack;
import net.minecraft.util.Identifier;
import org.lwjgl.opengl.GL11;

/** The core's drawing on old Minecraft's fixed-function OpenGL. */
final class LegacyGfx implements Gfx {
	/** The status effect icons (18 px cells from y = 198). */
	private static final Identifier INVENTORY = new Identifier("textures/gui/container/inventory.png");
	private static final int EFFECT_CELL = 18;
	private static final int EFFECT_Y = 198;
	private static final int EFFECTS_PER_ROW = 8;
	private static final Gradient GRADIENT = new Gradient();

	private final MinecraftClient mc = MinecraftClient.getInstance();
	private final Window window = new Window(mc);

	@Override
	public int width() {
		return (int) window.getScaledWidth();
	}

	@Override
	public int height() {
		return (int) window.getScaledHeight();
	}

	@Override
	public float pixelScale() {
		return window.getScaleFactor();
	}

	@Override
	public void fill(int x0, int y0, int x1, int y1, int color) {
		DrawableHelper.fill(x0, y0, x1, y1, color);
	}

	/**
	 * A rounded panel as one batch: every row of the corners, the middle and the faded edge pixels go out between a
	 * single begin and end, with the blend and texture state set once. Row by row through the game's own
	 * rectangle drawing it was dozens of rectangles of about ten state changes each per panel, which the HUD's
	 * panels added up to thousands of driver calls a frame.
	 */
	@Override
	public boolean roundedFill(int x0, int y0, int x1, int y1, int r, int color) {
		float alpha = (color >>> 24 & 255) / 255f;
		if (alpha <= 0f) {
			return true;
		}
		float red = (color >> 16 & 255) / 255f;
		float green = (color >> 8 & 255) / 255f;
		float blue = (color & 255) / 255f;
		GlStateManager.enableBlend();
		GlStateManager.disableTexture();
		GlStateManager.blendFuncSeparate(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA, GL11.GL_ONE, GL11.GL_ZERO);
		GL11.glBegin(GL11.GL_QUADS);
		GL11.glColor4f(red, green, blue, alpha);
		int radius = Math.max(0, Math.min(r, Math.min(x1 - x0, y1 - y0) / 2));
		for (int i = 0; i < radius; i++) {
			float dy = radius - i - 0.5f;
			float inset = radius - (float) Math.sqrt(Math.max(0f, radius * (float) radius - dy * dy));
			int full = (int) Math.ceil(inset);
			int top = y0 + i;
			int bottom = y1 - 1 - i;
			if (x1 - full > x0 + full) {
				quad(x0 + full, top, x1 - full, top + 1);
				quad(x0 + full, bottom, x1 - full, bottom + 1);
			}
			float coverage = full - inset;
			if (coverage > 0.04f && full >= 1) {
				GL11.glColor4f(red, green, blue, alpha * coverage);
				quad(x0 + full - 1, top, x0 + full, top + 1);
				quad(x1 - full, top, x1 - full + 1, top + 1);
				quad(x0 + full - 1, bottom, x0 + full, bottom + 1);
				quad(x1 - full, bottom, x1 - full + 1, bottom + 1);
				GL11.glColor4f(red, green, blue, alpha);
			}
		}
		if (y1 - radius > y0 + radius) {
			quad(x0, y0 + radius, x1, y1 - radius);
		}
		GL11.glEnd();
		GlStateManager.enableTexture();
		GlStateManager.disableBlend();
		GlStateManager.color(1f, 1f, 1f, 1f);
		return true;
	}

	private static void quad(int x0, int y0, int x1, int y1) {
		GL11.glVertex2f(x0, y1);
		GL11.glVertex2f(x1, y1);
		GL11.glVertex2f(x1, y0);
		GL11.glVertex2f(x0, y0);
	}

	@Override
	public void gradient(int x0, int y0, int x1, int y1, int top, int bottom) {
		GRADIENT.draw(x0, y0, x1, y1, top, bottom);
	}

	@Override
	public void text(String text, int x, int y, int color, boolean shadow) {
		if ((color >>> 24) == 0) {
			return;
		}
		mc.textRenderer.draw(text, (float) x, (float) y, color, shadow);
		GlStateManager.color(1f, 1f, 1f, 1f);
	}

	@Override
	public int textWidth(String text) {
		return mc.textRenderer.getStringWidth(text);
	}

	@Override
	public void texture(String key, int x, int y, int w, int h, float u, float v, int regionW, int regionH, int texW, int texH) {
		Identifier id = LegacyTextures.resolve(key);
		if (id == null) {
			return;
		}
		mc.getTextureManager().bindTexture(id);
		GlStateManager.enableBlend();
		GlStateManager.blendFuncSeparate(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA, GL11.GL_ONE, GL11.GL_ZERO);
		GlStateManager.color(1f, 1f, 1f, 1f);
		DrawableHelper.drawTexture(x, y, u, v, regionW, regionH, w, h, texW, texH);
	}

	@Override
	public void item(Object stack, int x, int y) {
		if (!(stack instanceof ItemStack)) {
			return;
		}
		GlStateManager.pushMatrix();
		DiffuseLighting.enable();
		GlStateManager.enableRescaleNormal();
		//#if MC >= 1.9
		mc.getItemRenderer().method_12461((ItemStack) stack, x, y);
		//#else
		mc.getItemRenderer().renderInGuiWithOverrides((ItemStack) stack, x, y);
		//#endif
		DiffuseLighting.disable();
		GlStateManager.disableRescaleNormal();
		GlStateManager.popMatrix();
	}

	@Override
	public void sprite(Object sprite, int x, int y, int w, int h) {
		if (!(sprite instanceof Integer)) {
			return;
		}
		int index = (Integer) sprite;
		mc.getTextureManager().bindTexture(INVENTORY);
		GlStateManager.enableBlend();
		GlStateManager.color(1f, 1f, 1f, 1f);
		float u = index % EFFECTS_PER_ROW * EFFECT_CELL;
		float v = EFFECT_Y + index / EFFECTS_PER_ROW * EFFECT_CELL;
		DrawableHelper.drawTexture(x, y, u, v, EFFECT_CELL, EFFECT_CELL, w, h, 256, 256);
	}

	@Override
	public void player(int x0, int y0, int x1, int y1, int scale, int mouseX, int mouseY) {
		LegacyPlayerPreview.draw(x0, y0, x1, y1, scale, mouseX, mouseY);
	}

	@Override
	public void push() {
		GlStateManager.pushMatrix();
	}

	@Override
	public void pop() {
		GlStateManager.popMatrix();
	}

	@Override
	public void translate(float x, float y) {
		GlStateManager.translate(x, y, 0f);
	}

	@Override
	public void scale(float s) {
		GlStateManager.scale(s, s, 1f);
	}

	@Override
	public void scissor(int x0, int y0, int x1, int y1) {
		int f = window.getScaleFactor();
		GL11.glEnable(GL11.GL_SCISSOR_TEST);
		GL11.glScissor(x0 * f, mc.height - y1 * f, Math.max(0, (x1 - x0) * f), Math.max(0, (y1 - y0) * f));
	}

	@Override
	public void endScissor() {
		GL11.glDisable(GL11.GL_SCISSOR_TEST);
	}

	/** DrawableHelper's gradient is protected; this reaches it. */
	private static final class Gradient extends DrawableHelper {
		void draw(int x0, int y0, int x1, int y1, int top, int bottom) {
			fillGradient(x0, y0, x1, y1, top, bottom);
		}
	}
}
