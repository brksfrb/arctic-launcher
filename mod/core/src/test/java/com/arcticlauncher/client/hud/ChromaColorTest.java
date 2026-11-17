package com.arcticlauncher.client.hud;

import java.awt.Color;
import java.lang.reflect.Method;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class ChromaColorTest {
    @Test void everyHueKeepsTheSameRgbWithoutTemporaryColorArrays() throws Exception {
        Method hsv = HudWidget.class.getDeclaredMethod("hsv", float.class, float.class, float.class);
        hsv.setAccessible(true);
        for (float saturation : new float[] {0f, .55f, 1f}) {
            for (float brightness : new float[] {0f, .7f, 1f}) {
                for (int i = 0; i < 4096; i++) {
                    float hue = i / 4096f;
                    assertEquals(Color.HSBtoRGB(hue, saturation, brightness) & 0xffffff,
                            ((Integer) hsv.invoke(null, hue, saturation, brightness)).intValue());
                }
            }
        }
    }
}
