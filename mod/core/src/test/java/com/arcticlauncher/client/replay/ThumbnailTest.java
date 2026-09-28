package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.nio.ByteBuffer;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;

class ThumbnailTest {
	/** A frame whose top half is red and bottom half blue (in screen order). */
	private static ByteBuffer frame(int w, int h, boolean bottomUp) {
		ByteBuffer b = ByteBuffer.allocate(w * h * 4);
		for (int row = 0; row < h; row++) {
			int screenY = bottomUp ? h - 1 - row : row;
			boolean top = screenY < h / 2;
			for (int x = 0; x < w; x++) {
				b.put((byte) (top ? 255 : 0)).put((byte) 0).put((byte) (top ? 0 : 255)).put((byte) 255);
			}
		}
		b.flip();
		return b;
	}

	private static BufferedImage decode(byte[] png) throws Exception {
		return ImageIO.read(new ByteArrayInputStream(png));
	}

	@Test
	void shrinksToThumbnailWidthKeepingTheShape() throws Exception {
		BufferedImage img = decode(Thumbnail.png(1280, 720, frame(1280, 720, false), false));
		assertEquals(320, img.getWidth());
		assertEquals(180, img.getHeight());
	}

	@Test
	void bottomUpFramesComeOutTheRightWayUp() throws Exception {
		BufferedImage img = decode(Thumbnail.png(640, 360, frame(640, 360, true), true));
		assertEquals(0xFF0000, img.getRGB(10, 10) & 0xFFFFFF);
		assertEquals(0x0000FF, img.getRGB(10, img.getHeight() - 10) & 0xFFFFFF);
	}

	@Test
	void smallFramesStayTheirSize() throws Exception {
		byte[] png = Thumbnail.png(100, 50, frame(100, 50, false), false);
		assertNotNull(png);
		assertEquals(100, decode(png).getWidth());
		assertNull(Thumbnail.png(0, 0, ByteBuffer.allocate(0), false));
	}
}
