package com.arcticlauncher.bedrock;

import static org.junit.jupiter.api.Assertions.*;
import java.awt.image.BufferedImage;
import java.util.concurrent.ConcurrentHashMap;
import org.junit.jupiter.api.Test;

class SoftwareGfxTest {
    @Test void transparentSurfaceTransformsClipsAndRgbaChannelsSurviveTheBridge() {
        try (SoftwareGfx g = new SoftwareGfx(32, 32, new ConcurrentHashMap<String, BufferedImage>())) {
            assertEquals(0, g.image().getRGB(0, 0));
            g.push();
            g.translate(8, 4);
            g.scissor(0, 0, 2, 2);
            g.fill(0, 0, 10, 10, 0xFF123456);
            g.endScissor();
            g.pop();
            assertEquals(0xFF123456, g.image().getRGB(8, 4));
            assertEquals(0, g.image().getRGB(10, 4));
            byte[] rgba = NativeBridge.rgba(g.image());
            int offset = (4 * 32 + 8) * 4;
            assertArrayEquals(new byte[] {0x12, 0x34, 0x56, (byte) 0xFF},
                    java.util.Arrays.copyOfRange(rgba, offset, offset + 4));
        }
    }

    @Test void keyboardMappingUsesGlfwCoreKeysAndWindowsPhysicalKeys() {
        assertEquals(com.arcticlauncher.client.Keys.RIGHT_SHIFT, NativeBridge.coreKey(0xA1));
        assertEquals(com.arcticlauncher.client.Keys.ESCAPE, NativeBridge.coreKey(27));
        assertEquals('C', BedrockPlatform.virtualKey("key.keyboard.c"));
        assertEquals(0xA4, BedrockPlatform.virtualKey("key.keyboard.left.alt"));
        assertEquals(-1, BedrockPlatform.virtualKey("key.keyboard.unknown"));
    }
}
