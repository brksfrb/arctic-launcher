package com.arcticlauncher.client.gfx;

/**
 * The drawing primitives the core needs, in GUI-scaled units. Each version
 * adapter implements this on top of its own renderer. Colors are ARGB.
 */
public interface Gfx {
	int width();

	int height();

	void fill(int x0, int y0, int x1, int y1, int color);

	void gradient(int x0, int y0, int x1, int y1, int top, int bottom);

	/** Text in Minecraft's font; § formatting codes work. */
	void text(String text, int x, int y, int color, boolean shadow);

	int textWidth(String text);

	/**
	 * Draw part of a texture: {@code "icon"} (the Arctic snowflake) or
	 * {@code "look:<hash>"} (a downloaded look texture).
	 */
	void texture(String key, int x, int y, int w, int h, float u, float v, int regionW, int regionH, int texW, int texH);

	/** An item icon (16×16) with its count and durability bar; {@code stack} is the game's. */
	void item(Object stack, int x, int y);

	/** A GUI sprite from the game's atlas (like an effect icon); {@code sprite} is the game's id. */
	void sprite(Object sprite, int x, int y, int w, int h);

	void push();

	void pop();

	void translate(float x, float y);

	void scale(float s);

	void scissor(int x0, int y0, int x1, int y1);

	void endScissor();
}
