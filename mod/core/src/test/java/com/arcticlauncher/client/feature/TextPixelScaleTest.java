package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import java.nio.FloatBuffer;
import org.junit.jupiter.api.Test;

class TextPixelScaleTest {
	@Test void guiScaleAndNonuniformTransforms() {
		// Orthographic 640x360 logical GUI on a 1280x720 framebuffer, title scaled by four.
		float[] clip = {8f / 640, 0, 0, 0, 0, -8f / 360, 0, 0, 0, 0, 1, 0, -1, 1, 0, 1};
		assertEquals(8, TextPixelScale.projected(clip, 120, 30, 1280, 720), 0.001);
		// An area/determinant estimate would miss this 8x stretch entirely.
		assertEquals(8, TextPixelScale.stretch(8, 0, 0, 0.125f), 0.001);
		assertEquals(4, TextPixelScale.stretch(0, 4, -4, 0), 0.001);
	}

	@Test void worldDistanceFovViewportAndLabelScale() {
		FloatBuffer projection = FloatBuffer.wrap(new float[] {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -1, -1, 0, 0, -0.1f, 0});
		FloatBuffer model = FloatBuffer.wrap(new float[] {0.025f, 0, 0, 0, 0, 0.025f, 0, 0, 0, 0, 1, 0, 0, 0, -1, 1});
		float[] clip = new float[16];
		TextPixelScale.multiply(projection, model, clip);
		assertEquals(9, TextPixelScale.projected(clip, 0, 0, 720, 720), 0.001);
		model.put(14, -4);
		TextPixelScale.multiply(projection, model, clip);
		assertEquals(2.25, TextPixelScale.projected(clip, 0, 0, 720, 720), 0.001);
		assertEquals(4.5, TextPixelScale.projected(clip, 0, 0, 1440, 1440), 0.001);
		projection.put(0, 2).put(5, 2);
		TextPixelScale.multiply(projection, model, clip);
		assertEquals(4.5, TextPixelScale.projected(clip, 0, 0, 720, 720), 0.001);
	}

	@Test void perspectiveTiltUsesAnchorAndStaysFiniteAtCameraPlane() {
		float[] clip = {0.02f, 0, 0, 0.001f, 0, 0.02f, 0, 0, 0, 0, -1, -1, 0, 0, 0, 1};
		float near = TextPixelScale.projected(clip, -100, 0, 720, 720);
		float far = TextPixelScale.projected(clip, 100, 0, 720, 720);
		assertTrue(near > far);
		clip[15] = 0;
		assertEquals(Float.MAX_VALUE, TextPixelScale.projected(clip, 0, 0, 720, 720));
	}
}
