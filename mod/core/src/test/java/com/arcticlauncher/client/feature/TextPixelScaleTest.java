package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.assertFalse;
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

	@Test void rasterSizeTracksGuiScaleAndQuantizesPerPixel() {
		assertEquals(16, Badge.rasterSize(false, 2f));
		assertEquals(24, Badge.rasterSize(false, 3f));
		assertEquals(32, Badge.rasterSize(false, 4f));
		assertEquals(18, Badge.rasterSize(true, 2f));
		assertEquals(27, Badge.rasterSize(true, 3f));
		assertEquals(36, Badge.rasterSize(true, 4f));
		assertEquals(27, Badge.rasterSize(true, 3.04f));
		assertEquals(27, Badge.rasterSize(true, 2.96f));
		assertEquals(28, Badge.rasterSize(true, 3.12f));
		assertEquals(1, Badge.rasterSize(false, 0.01f));
		assertEquals(Badge.MAX_RASTER_SIZE, Badge.rasterSize(false, 1000f));
	}

	@Test void closerCameraAndNarrowerFovYieldLargerRasters() {
		FloatBuffer projection = FloatBuffer.wrap(new float[] {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -1, -1, 0, 0, -0.1f, 0});
		FloatBuffer model = FloatBuffer.wrap(new float[] {0.025f, 0, 0, 0, 0, 0.025f, 0, 0, 0, 0, 1, 0, 0, 0, -1, 1});
		float[] clip = new float[16];
		TextPixelScale.multiply(projection, model, clip);
		float near = TextPixelScale.projected(clip, 0, 0, 720, 720);
		model.put(14, -4);
		TextPixelScale.multiply(projection, model, clip);
		float far = TextPixelScale.projected(clip, 0, 0, 720, 720);
		assertTrue(near > far * 3.5f, "near=" + near + " far=" + far);
		assertTrue(Badge.rasterSize(false, near) > Badge.rasterSize(false, far));
		// Doubling the projection scale is a narrower FOV / Arctic zoom; sprint FOV does the opposite.
		projection.put(0, 2).put(5, 2);
		TextPixelScale.multiply(projection, model, clip);
		float zoom = TextPixelScale.projected(clip, 0, 0, 720, 720);
		assertTrue(zoom > far * 1.9f, "zoom=" + zoom + " far=" + far);
		assertTrue(Badge.rasterSize(false, zoom) > Badge.rasterSize(false, far));
	}

	@Test void snapMovesAxisAlignedOriginOntoWholePixels() {
		float[] clip = TextPixelScale.orthoPixels(1920, 1080);
		float[] out = new float[2];
		assertTrue(TextPixelScale.snapOrigin(clip, 10.4f, 20.6f, 1920, 1080, out));
		assertEquals(10f, out[0], 1e-4);
		assertEquals(21f, out[1], 1e-4);
		float[] fb = new float[2];
		TextPixelScale.projectPoint(clip, out[0], out[1], 1920, 1080, fb);
		assertEquals(10f, fb[0], 1e-4);
		assertEquals(21f, fb[1], 1e-4);
	}

	@Test void snappedQuadCornersLandOnWholePixels() {
		float[] clip = TextPixelScale.orthoPixels(1920, 1080);
		float[] origin = new float[2];
		assertTrue(TextPixelScale.snapOrigin(clip, 40.3f, 80.7f, 1920, 1080, origin));
		float side = 9f;
		float[] fb = new float[2];
		TextPixelScale.projectPoint(clip, origin[0], origin[1], 1920, 1080, fb);
		assertEquals(Math.round(fb[0]), fb[0], 1e-3);
		assertEquals(Math.round(fb[1]), fb[1], 1e-3);
		TextPixelScale.projectPoint(clip, origin[0] + side, origin[1] + side, 1920, 1080, fb);
		assertEquals(Math.round(fb[0]), fb[0], 1e-3);
		assertEquals(Math.round(fb[1]), fb[1], 1e-3);
	}

	@Test void rotatedQuadIsLeftUnsnapped() {
		float[] clip = TextPixelScale.orthoPixels(1920, 1080);
		double a = Math.toRadians(20);
		float c = (float) Math.cos(a), s = (float) Math.sin(a);
		float[] rot = new float[16];
		System.arraycopy(clip, 0, rot, 0, 16);
		float o0 = clip[0], o5 = clip[5];
		rot[0] = o0 * c;
		rot[1] = o5 * s;
		rot[4] = o0 * -s;
		rot[5] = o5 * c;
		float[] out = new float[] {99, 99};
		assertFalse(TextPixelScale.snapOrigin(rot, 10.4f, 20.6f, 1920, 1080, out));
		assertEquals(99f, out[0], 0f);
	}

	@Test void worldBadgeKeepsItsTextSpaceSizeAcrossZoomAndDistance() {
		for (float scale : new float[] {0.01f, 0.7f, 3.4f, 16f, 32f, 1000f, Float.MAX_VALUE}) {
			assertEquals(8f, Badge.quadSide(false, scale));
		}
	}

	@Test void tabBadgeKeepsWholeFramebufferPixels() {
		assertEquals(31 / 3.4f, Badge.quadSide(true, 3.4f), 1e-6);
		assertEquals(9f, Badge.quadSide(true, 3f), 1e-6);
		assertEquals(9f, Badge.quadSide(true, 16f), 1e-6);
		assertEquals(Badge.MAX_RASTER_SIZE, Badge.rasterSize(true, 16f));
	}
}
