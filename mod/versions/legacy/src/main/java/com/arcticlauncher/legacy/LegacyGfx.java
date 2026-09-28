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
