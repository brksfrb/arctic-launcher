package com.arcticlauncher.client.gfx;

/**
 * The drawing primitives the core needs, in GUI-scaled units. Each version
 * adapter implements this on top of its own renderer. Colors are ARGB.
 */
public interface Gfx {
	int width();

	/** Screen pixels per GUI pixel (the GUI scale). */
	float pixelScale();

	int height();

	void fill(int x0, int y0, int x1, int y1, int color);

	void gradient(int x0, int y0, int x1, int y1, int top, int bottom);

	/** Text in Minecraft's font; § formatting codes work. */
	void text(String text, int x, int y, int color, boolean shadow);

	int textWidth(String text);

	/**
	 * Draw part of a texture: {@code "icon"} (the Arctic snowflake),
	 * {@code "look:<hash>"} (a downloaded look texture) or
	 * {@code "asset:<name>"} ({@code assets/arctic/<name>.png} in the jar).
	 */
	void texture(String key, int x, int y, int w, int h, float u, float v, int regionW, int regionH, int texW, int texH);

	/** An item icon (16×16) with its count and durability bar; {@code stack} is the game's. */
	void item(Object stack, int x, int y);

	/** A GUI sprite from the game's atlas (like an effect icon); {@code sprite} is the game's id. */
	void sprite(Object sprite, int x, int y, int w, int h);

	/**
	 * The local player's model in the box (x0, y0)-(x1, y1), looking toward
	 * the mouse, with everything they wear. Nothing outside a world.
	 */
	void player(int x0, int y0, int x1, int y1, int scale, int mouseX, int mouseY);

	void push();

	void pop();

	void translate(float x, float y);

	void scale(float s);

	void scissor(int x0, int y0, int x1, int y1);

	void endScissor();

	/**
	 * A filled rounded rectangle in physical pixels (under a 1/{@link #pixelScale}
	 * transform), drawn as a few pieces if this renderer can; false to have
	 * it drawn row by row instead.
	 */
	/**
	 * Start keeping the GUI pieces drawn from now on, to draw them again
	 * unchanged later ({@link #replay}): a HUD widget showing the same thing
	 * skips its drawing (and the game's text shaping) entirely. False if this
	 * game version can't.
	 */
	default boolean startRecording() {
		return false;
	}

	/** The pieces drawn since {@link #startRecording} (null if it wasn't). */
	default Object stopRecording() {
		return null;
	}

	/** Draw recorded pieces again, where they were; false if this game version can't. */
	default boolean replay(Object recorded) {
		return false;
	}

	/**
	 * What's drawn next goes in a new layer above everything so far. The
	 * game's GUI (1.21.6+) compares every new piece with all the pieces in
	 * the current layer to stack overlapping ones in order; a busy HUD on
	 * top of the game's own paid for that with every piece. Drawing on top
	 * anyway, a widget in its own layer only meets its own pieces.
	 */
	default void newLayer() {}

	default boolean roundedFill(int x0, int y0, int x1, int y1, int radius, int color) {
		return false;
	}
}
