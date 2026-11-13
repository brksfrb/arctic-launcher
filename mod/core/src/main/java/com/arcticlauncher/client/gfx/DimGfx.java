package com.arcticlauncher.client.gfx;

/** Draws through another {@link Gfx} with every color's alpha multiplied (a faded panel). */
public final class DimGfx implements Gfx {
	private final Gfx g;
	private final float a;

	public DimGfx(Gfx g, float a) {
		this.g = g;
		this.a = a;
	}

	@Override
	public int width() {
		return g.width();
	}

	@Override
	public float pixelScale() {
		return g.pixelScale();
	}

	@Override
	public int height() {
		return g.height();
	}

	@Override
	public void fill(int x0, int y0, int x1, int y1, int color) {
		g.fill(x0, y0, x1, y1, Draw.alpha(color, a));
	}

	@Override
	public void gradient(int x0, int y0, int x1, int y1, int top, int bottom) {
		g.gradient(x0, y0, x1, y1, Draw.alpha(top, a), Draw.alpha(bottom, a));
	}

	@Override
	public void text(String text, int x, int y, int color, boolean shadow) {
		g.text(text, x, y, Draw.alpha(color, a), shadow);
	}

	@Override
	public int textWidth(String text) {
		return g.textWidth(text);
	}

	@Override
	public void texture(String key, int x, int y, int w, int h, float u, float v, int regionW, int regionH, int texW, int texH) {
		// Textures can't be faded here: leave them out of a dimmed card.
	}

	@Override
	public void item(Object stack, int x, int y) {
		g.item(stack, x, y);
	}

	@Override
	public void sprite(Object sprite, int x, int y, int w, int h) {
		g.sprite(sprite, x, y, w, h);
	}

	@Override
	public void player(int x0, int y0, int x1, int y1, int scale, int mouseX, int mouseY) {
		g.player(x0, y0, x1, y1, scale, mouseX, mouseY);
	}

	@Override
	public void push() {
		g.push();
	}

	@Override
	public void pop() {
		g.pop();
	}

	@Override
	public void translate(float x, float y) {
		g.translate(x, y);
	}

	@Override
	public void scale(float s) {
		g.scale(s);
	}

	@Override
	public void rotate(float degrees) {
		g.rotate(degrees);
	}

	@Override
	public void scissor(int x0, int y0, int x1, int y1) {
		g.scissor(x0, y0, x1, y1);
	}

	@Override
	public void endScissor() {
		g.endScissor();
	}

	@Override
	public boolean roundedFill(int x0, int y0, int x1, int y1, int radius, int color) {
		return g.roundedFill(x0, y0, x1, y1, radius, Draw.alpha(color, a));
	}
}
