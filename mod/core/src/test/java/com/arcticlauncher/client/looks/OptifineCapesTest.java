package com.arcticlauncher.client.looks;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;

class OptifineCapesTest {
	private static byte[] png(int w, int h) throws Exception {
		BufferedImage image = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
		image.setRGB(0, 0, 0xFFFF0000);
		ByteArrayOutputStream out = new ByteArrayOutputStream();
		ImageIO.write(image, "png", out);
		return out.toByteArray();
	}

	@Test
	void optifinesOwnLayoutMovesIntoMinecraftsTexture() throws Exception {
		byte[] moved = OptifineCapes.layout(png(46, 22));
		BufferedImage image = ImageIO.read(new ByteArrayInputStream(moved));
		assertEquals(64, image.getWidth());
		assertEquals(32, image.getHeight());
		assertEquals(0xFFFF0000, image.getRGB(0, 0));
		assertTrue(Looks.fitsTexture(moved, true));
	}

	@Test
	void aBiggerOptifineCapeKeepsItsScale() throws Exception {
		BufferedImage image = ImageIO.read(new ByteArrayInputStream(OptifineCapes.layout(png(92, 44))));
		assertEquals(128, image.getWidth());
		assertEquals(64, image.getHeight());
	}

	@Test
	void aMinecraftSizedCapeIsLeftAlone() throws Exception {
		byte[] plain = png(64, 32);
		assertSame(plain, OptifineCapes.layout(plain));
	}

	@Test
	void somethingThatIsNotAPngIsRefused() {
		try {
			OptifineCapes.layout(new byte[] {1, 2, 3});
			throw new AssertionError("expected a failure");
		} catch (java.io.IOException expected) {
			// fine
		}
	}
}
