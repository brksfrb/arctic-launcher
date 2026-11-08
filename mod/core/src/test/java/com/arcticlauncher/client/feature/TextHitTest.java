package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

import org.junit.jupiter.api.Test;

class TextHitTest {
	private static final float BOLD = 0.35f;

	@Test
	void aSpaceOfTwoAndAHalfBecomesThreeWholePixels() {
		assertEquals(3, TextHit.step(2.5f));
		// The 1.8 renderer casts the glyph's return to int. 2.5 becomes 2, one pixel left of the step.
		assertEquals(2, TextHit.penAdvance(2.5f, false));
		assertEquals(3, TextHit.penAdvance(TextHit.glyphReturn(TextHit.step(2.5f), false, BOLD), false));
	}

	@Test
	void boldThickenDoesNotChangeThePenStep() {
		int step = TextHit.step(5.5f);
		float returned = TextHit.glyphReturn(step, true, BOLD);
		assertEquals(step, TextHit.penAdvance(returned, true));
		assertEquals(step, TextHit.penAdvance(TextHit.glyphReturn(step, false, BOLD), false));
	}

	@Test
	void fractionalAdvancesDriftOntoTheWrongComponent() {
		// Three buttons. Each glyph advance is x.5, so a cast to int drops a pixel per glyph
		// and the ceil of the raw sum (the old width used for clicks) does not land on the same button.
		float[] glyphs = {5.5f, 5.5f, 2.5f};
		int[] drawn = new int[3];
		int[] measured = new int[3];
		for (int i = 0; i < glyphs.length; i++) {
			int trunc = 0;
			float exact = 0f;
			for (int n = 0; n < 3; n++) {
				trunc += (int) glyphs[i];
				exact += glyphs[i];
			}
			drawn[i] = trunc;
			measured[i] = (int) Math.ceil(exact);
		}
		int[] aligned = new int[3];
		for (int i = 0; i < glyphs.length; i++) {
			aligned[i] = 3 * TextHit.step(glyphs[i]);
		}
		// Where the third button was drawn (the cast-to-int pen). The ceil'd widths miss it.
		int drawnClick = drawn[0] + drawn[1] + drawn[2] / 2;
		assertEquals(2, TextHit.componentAt(drawnClick, drawn));
		assertNotEquals(2, TextHit.componentAt(drawnClick, measured));
		// Once drawing uses the same whole-pixel step as the widths, that button's own centre hits it.
		int alignedClick = aligned[0] + aligned[1] + aligned[2] / 2;
		assertEquals(2, TextHit.componentAt(alignedClick, aligned));
	}

	@Test
	void lineCentresStayOnTheirLineOnlyAtTheFontStride() {
		int stride = 11;
		int vanilla = 9;
		// Centre of the third line when lines are `stride` apart.
		int y = 2 * stride + stride / 2;
		assertEquals(2, TextHit.lineOf(y, stride, 0, 10));
		assertEquals(3, TextHit.lineOf(y, vanilla, 0, 10));
	}

	@Test
	void theGapInsideALineDoesNotSelectTheNextOne() {
		int stride = 11;
		assertEquals(0, TextHit.lineOf(0, stride, 0, 5));
		assertEquals(0, TextHit.lineOf(stride - 1, stride, 0, 5));
		assertEquals(1, TextHit.lineOf(stride, stride, 0, 5));
	}

	@Test
	void scrollingShiftsTheLineByTheSameCountTheChatUses() {
		assertEquals(4, TextHit.lineOf(11 + 3, 11, 3, 20));
		assertEquals(-1, TextHit.lineOf(-1, 11, 0, 5));
		assertEquals(-1, TextHit.lineOf(0, 11, 0, 0));
	}

	@Test
	void aClickOnTheBoundaryBelongsToTheNextComponent() {
		int[] widths = {10, 4, 10};
		assertEquals(0, TextHit.componentAt(0, widths));
		assertEquals(0, TextHit.componentAt(9, widths));
		assertEquals(1, TextHit.componentAt(10, widths));
		assertEquals(2, TextHit.componentAt(14, widths));
		assertEquals(-1, TextHit.componentAt(24, widths));
		assertEquals(-1, TextHit.componentAt(-1, widths));
	}
}
