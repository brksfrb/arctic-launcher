package com.arcticlauncher.client.looks;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

import java.awt.image.BufferedImage;
import org.junit.jupiter.api.Test;

class SkinFormatTest {
	@Test
	void anOldSkinGetsALeftLegAndArm() {
		BufferedImage old = new BufferedImage(64, 32, BufferedImage.TYPE_INT_ARGB);
		// The right leg's front (4,20) and the right arm's front (44,20).
		old.setRGB(4, 20, 0xFF112233);
		old.setRGB(44, 20, 0xFF445566);
		BufferedImage out = SkinFormat.modern(old);
		assertEquals(64, out.getHeight());
		// Mirrored into the left leg's front (20..24, 52..64) and left arm's front (36..40, 52..64).
		assertEquals(0xFF112233, out.getRGB(23, 52));
		assertEquals(0xFF445566, out.getRGB(39, 52));
	}

	@Test
	void theBaseLayerIsOpaqueAndAFullySolidHatIsCleared() {
		BufferedImage skin = new BufferedImage(64, 64, BufferedImage.TYPE_INT_ARGB);
		for (int x = 32; x < 64; x++) {
			for (int y = 0; y < 32; y++) {
				skin.setRGB(x, y, 0xFF00FF00);
			}
		}
		skin.setRGB(8, 8, 0x00FF0000);
		BufferedImage out = SkinFormat.modern(skin);
		assertEquals(0xFFFF0000, out.getRGB(8, 8));
		assertEquals(0, out.getRGB(40, 8) >>> 24);
	}

	@Test
	void hdSkinsScaleAndOtherSizesAreLeftAlone() {
		assertEquals(256, SkinFormat.modern(new BufferedImage(256, 128, BufferedImage.TYPE_INT_ARGB)).getHeight());
		assertNull(SkinFormat.modern(new BufferedImage(64, 48, BufferedImage.TYPE_INT_ARGB)));
	}
}
