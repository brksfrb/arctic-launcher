package com.arcticlauncher.legacy;

import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.MenuFont;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawableHelper;
import net.minecraft.client.render.DiffuseLighting;
import net.minecraft.client.util.Window;
import net.minecraft.item.ItemStack;
import net.minecraft.util.Identifier;
import org.lwjgl.opengl.GL11;

/** The core's drawing on old Minecraft's fixed-function OpenGL. */
final class LegacyGfx implements Gfx, MenuFont.DrawGlyph {
	/** The status effect icons (18 px cells from y = 198). */
	private static final Identifier INVENTORY = new Identifier("textures/gui/container/inventory.png");
	private static final int EFFECT_CELL = 18;
	private static final int EFFECT_Y = 198;
	private static final int EFFECTS_PER_ROW = 8;
	private static final Gradient GRADIENT = new Gradient();

	private final MinecraftClient mc = MinecraftClient.getInstance();
	private Window window = new Window(mc);
	private boolean useMenuText;
	private float opacity = 1f;
	private static final Identifier[] MENU_ATLASES = new Identifier[4];

	void init() {
		window = new Window(mc);
	}

	@Override
	public boolean menuText(boolean on) {
		boolean before = useMenuText;
		useMenuText = on;
		return before;
	}

	@Override
	public float opacity(float value) {
		float before = opacity;
		opacity = value;
		return before;
	}

	private int tinted(int color) {
		return opacity == 1f ? color : Draw.alpha(color, opacity);
	}
	private boolean filling;

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
		if (filling) {
			quad(x0, y0, x1, y1);
		} else {
			DrawableHelper.fill(x0, y0, x1, y1, tinted(color));
		}
	}

	@Override
	public void beginFills(int color) {
		color = tinted(color);
		GlStateManager.enableBlend();
		GlStateManager.disableTexture();
		GlStateManager.blendFuncSeparate(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA, GL11.GL_ONE, GL11.GL_ZERO);
		GlStateManager.color((color >> 16 & 255) / 255f, (color >> 8 & 255) / 255f,
				(color & 255) / 255f, (color >>> 24) / 255f);
		GL11.glBegin(GL11.GL_QUADS);
		filling = true;
	}

	@Override
	public void endFills() {
		filling = false;
		GL11.glEnd();
		GlStateManager.enableTexture();
		GlStateManager.disableBlend();
	}

	/**
	 * A rounded panel as one batch: every row of the corners, the middle and the faded edge pixels go out between a
	 * single begin and end, with the blend and texture state set once. Row by row through the game's own
	 * rectangle drawing it was dozens of rectangles of about ten state changes each per panel, which the HUD's
	 * panels added up to thousands of driver calls a frame.
	 */
	@Override
	public boolean roundedFill(int x0, int y0, int x1, int y1, int r, int color) {
		color = tinted(color);
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
		GlStateManager.color(red, green, blue, alpha);
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
				GlStateManager.color(red, green, blue, alpha * coverage);
				quad(x0 + full - 1, top, x0 + full, top + 1);
				quad(x1 - full, top, x1 - full + 1, top + 1);
				quad(x0 + full - 1, bottom, x0 + full, bottom + 1);
				quad(x1 - full, bottom, x1 - full + 1, bottom + 1);
				GlStateManager.color(red, green, blue, alpha);
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
		GRADIENT.draw(x0, y0, x1, y1, tinted(top), tinted(bottom));
	}

	@Override
	public void text(String text, int x, int y, int color, boolean shadow) {
		color = tinted(color);
		// The game's font draws a colour with alpha 1-3 fully opaque: skip what could not be seen anyway.
		if ((color >>> 24) < 4) {
			return;
		}
		MenuFont menu = useMenuText ? MenuFont.at(pixelScale()) : null;
		if (menu != null && menu.width(text) >= 0 && drawMenuText(menu, text, x, y, color, shadow)) return;
		mc.textRenderer.draw(text, (float) x, (float) y, color, shadow);
		GlStateManager.color(1f, 1f, 1f, 1f);
	}

	@Override
	public int textWidth(String text) {
		MenuFont menu = useMenuText ? MenuFont.at(pixelScale()) : null;
		int width = menu == null ? -1 : menu.width(text);
		return width >= 0 ? width : mc.textRenderer.getStringWidth(text);
	}

	private boolean drawMenuText(MenuFont font, String text, int x, int y, int color, boolean shadow) {
		int index = font.scale() - 1;
		Identifier id = MENU_ATLASES[index];
		if (id == null) {
			id = new Identifier("arctic", "menu_font/" + font.scale());
			if (!LegacyTextures.register(id, font.image())) return false;
			MENU_ATLASES[index] = id;
		}
		mc.getTextureManager().bindTexture(id);
		GlStateManager.enableBlend();
		GlStateManager.blendFuncSeparate(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA, GL11.GL_ONE, GL11.GL_ZERO);
		GL11.glBegin(GL11.GL_QUADS);
		font.draw(this, text, x, y, color, shadow);
		GL11.glEnd();
		GL11.glColor4f(1f, 1f, 1f, 1f);
		GlStateManager.color(1f, 1f, 1f, 1f);
		return true;
	}

	@Override
	public void glyph(int x, int y, int w, int h, int u, int v, int scale, int imageW, int imageH, int color) {
		GL11.glColor4ub((byte) (color >>> 16), (byte) (color >>> 8), (byte) color, (byte) (color >>> 24));
		float u0 = (float) u / imageW, v0 = (float) v / imageH;
		float u1 = (float) (u + w * scale) / imageW, v1 = (float) (v + h * scale) / imageH;
		GL11.glTexCoord2f(u0, v1);
		GL11.glVertex2f(x, y + h);
		GL11.glTexCoord2f(u1, v1);
		GL11.glVertex2f(x + w, y + h);
		GL11.glTexCoord2f(u1, v0);
		GL11.glVertex2f(x + w, y);
		GL11.glTexCoord2f(u0, v0);
		GL11.glVertex2f(x, y);
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
		GlStateManager.color(1f, 1f, 1f, opacity);
		DrawableHelper.drawTexture(x, y, u, v, regionW, regionH, w, h, texW, texH);
		if (opacity < 1f) GlStateManager.color(1f, 1f, 1f, 1f);
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
	public void rotate(float degrees) {
		GlStateManager.rotate(degrees, 0f, 0f, 1f);
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
