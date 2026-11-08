package com.arcticlauncher.client.feature;

/**
 * Chat and book hit-testing, in the same units the text is drawn in.
 *
 * <p>On 1.8.9 the renderer asks a glyph how far to move, then keeps only the whole pixels
 * ({@code x += (int) advance}, and for bold {@code x += (int) (advance + 1)}). A fractional
 * advance (Inter's space is 2.5) is therefore a different width on screen than the sum of the
 * raw advances. Line selection divides by the line height. Both have to be the numbers the
 * renderer actually used.
 */
public final class TextHit {
	private TextHit() {}

	/** Whole pixels the pen should move. {@code Math.round} so 2.5 (a space) is 3, not the 2 left by a cast to int. */
	public static int step(float advance) {
		return Math.round(advance);
	}

	/**
	 * What 1.8's renderer adds to the pen: the glyph method's return value, plus one when the
	 * character is bold, then the whole pixels of that.
	 */
	public static int penAdvance(float glyphReturn, boolean bold) {
		float kept = bold ? glyphReturn + 1f : glyphReturn;
		return (int) kept;
	}

	/**
	 * The value to return from the glyph method so the pen moves by {@code step} pixels.
	 * Bold adds one before the cast to int; a thicken below one pixel must not change that step,
	 * so the return is pulled back by the same one the renderer adds.
	 */
	public static float glyphReturn(int step, boolean bold, float boldThicken) {
		if (!bold) {
			return step;
		}
		return step - (1f - boldThicken);
	}

	/**
	 * The line a chat-local y falls in. {@code y} is measured down from the top of the newest
	 * line's cell (the game's own direction: 0 is the bottom line). {@code scroll} is how many
	 * lines the history has been moved up. Returns -1 when y is above the lines that exist.
	 */
	public static int lineOf(int y, int lineHeight, int scroll, int messageCount) {
		if (lineHeight <= 0 || y < 0 || messageCount <= 0) {
			return -1;
		}
		int line = y / lineHeight + scroll;
		if (line < 0 || line >= messageCount) {
			return -1;
		}
		return line;
	}

	/**
	 * Which component contains {@code x}. {@code widths} are the pen widths, in order. The game
	 * keeps a component while the running width is still greater than x, so x equal to the end
	 * of a component belongs to the next one.
	 */
	public static int componentAt(int x, int[] widths) {
		if (widths == null || x < 0) {
			return -1;
		}
		int pen = 0;
		for (int i = 0; i < widths.length; i++) {
			pen += widths[i];
			if (pen > x) {
				return i;
			}
		}
		return -1;
	}
}
