package com.arcticlauncher.bedrock;

import static org.junit.jupiter.api.Assertions.*;

import com.arcticlauncher.client.ArcticClient;
import java.io.File;
import java.nio.file.Files;
import org.junit.jupiter.api.Test;

/** The native presentation path: nothing outside a world, the HUD only in play, the shared HUD editor on Right Shift. */
class PresentationTest {
    private static final boolean[] KEYS = new boolean[256];

    private static byte[] frame(int... events) {
        return NativeBridge.frameIfChanged(320, 180, 60, 10, 10, KEYS, events);
    }

    private static boolean anyPixel(byte[] rgba) {
        for (int i = 3; i < rgba.length; i += 4) if (rgba[i] != 0) return true;
        return false;
    }

    @Test
    void presentsOnlyWhatBelongsOnScreen() throws Exception {
        System.setProperty("arctic.cosmetics.url", "http://127.0.0.1:9");
        File config = Files.createTempDirectory("arctic-bedrock-presentation").toFile();
        NativeBridge.init(config.getAbsolutePath(), "1.26.65", "ArcticTest");
        BedrockPlatform platform = (BedrockPlatform) ArcticClient.platform();

        // Main menu / loading: no world, so nothing is rasterized or uploaded.
        NativeBridge.gameState(true, false, false, null, "start_screen");
        assertEquals(0, frame().length, "outside a world there is nothing to draw");
        assertFalse(platform.inWorld());
        assertNull(platform.position());

        // Playing with no game screen: the HUD is drawn and the position is the engine's.
        NativeBridge.gameState(true, true, false, new double[] {1.5, 64, -2, 90, 10}, "hud_screen");
        assertTrue(platform.inWorld());
        assertTrue(platform.noScreen());
        assertArrayEquals(new double[] {1.5, 64, -2, 90, 10}, platform.position());
        byte[] hud = frame();
        assertNotNull(hud);
        assertEquals(320 * 180 * 4, hud.length);

        // A game screen (inventory) over the world: the HUD steps aside.
        NativeBridge.gameState(true, true, true, new double[] {1.5, 64, -2, 90, 10}, "inventory_screen");
        assertFalse(platform.noScreen());
        assertEquals(0, frame().length, "a game screen hides the Arctic HUD");

        // Right Shift in play opens the shared HUD editor (drag widgets), with the status inside it only.
        NativeBridge.gameState(true, true, false, new double[] {1.5, 64, -2, 90, 10}, "hud_screen");
        NativeBridge.bindingStatus("screens ok");
        frame(0, 0xA1);
        assertTrue(NativeBridge.menuOpen());
        assertTrue(platform.page instanceof com.arcticlauncher.client.menu.HudEditor, "Right Shift opens the HUD editor");
        byte[] menu = frame();
        assertTrue(menu == null || anyPixel(menu));

        // Typing a capital letter in the game's chat doesn't open the menu.
        frame(0, 27);
        assertFalse(NativeBridge.menuOpen());
        NativeBridge.gameState(true, true, true, null, "chat_screen");
        frame(0, 0xA1);
        assertFalse(NativeBridge.menuOpen(), "Right Shift while typing in chat");

        // The font toggle isn't offered: Arctic doesn't own Bedrock's text.
        assertFalse(platform.gameFontWorks());
        assertTrue(platform.hudEditorWorks());
    }
}
