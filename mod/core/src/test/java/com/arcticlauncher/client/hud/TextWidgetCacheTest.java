package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class TextWidgetCacheTest {
    static class Label extends TextWidget {
        String value = "144";
        Label() { super("fps", "FPS", "", true); }
        protected String value(boolean preview) { return preview ? "sample" : value; }
    }
    private static void nextFrame() throws Exception {
        Field frame = Hud.class.getDeclaredField("frame");
        frame.setAccessible(true);
        frame.setLong(null, frame.getLong(null) + 1);
    }
    @Test void contentReusedUntilValueOrMutableLookChanges() throws Exception {
        Label w = new Label();
        HudSlot slot = new HudSlot(true);
        w.use(slot);
        Object first = w.content(false);
        nextFrame();
        assertSame(first, w.content(false));
        slot.brackets = true;
        assertNotEquals(first, w.content(false));
        assertTrue(w.content(false).toString().contains("[FPS: "));
        slot.labelMode = HudSlot.LABEL_AFTER;
        assertTrue(w.content(false).toString().contains("144 "));
        assertTrue(w.content(true).toString().contains("sample"));
        assertTrue(w.content(false).toString().contains("144"));
        w.value = "145";
        nextFrame();
        assertTrue(w.content(false).toString().contains("145"));
        slot.chroma = true;
        assertNull(w.content(false));
    }
    @Test void ownLabelsKeepTokensColorsAndRemeasureAtNewScale() throws Exception {
        Label w = new Label();
        w.ownLabels();
        w.value = "X: 4  Y: 70 Z: -3";
        HudSlot slot = new HudSlot(true);
        slot.labelColor = 0xff123456;
        w.use(slot);
        List<String> draws = new ArrayList<String>();
        int[] measures = {0};
        int[] glyphWidth = {6};
        List<Integer> positions = new ArrayList<Integer>();
        float[] scale = {1f};
        Gfx g = (Gfx) Proxy.newProxyInstance(Gfx.class.getClassLoader(), new Class<?>[] {Gfx.class}, (p, m, a) -> {
            if (m.getName().equals("pixelScale")) return scale[0];
            if (m.getName().equals("textWidth")) { measures[0]++; return ((String) a[0]).length() * glyphWidth[0]; }
            if (m.getName().equals("text")) { draws.add(a[0] + ":" + a[3]); positions.add((Integer) a[1]); }
            if (m.getReturnType() == int.class) return 720;
            if (m.getReturnType() == boolean.class) return false;
            return null;
        });
        w.render(g, Style.ARCTIC, false);
        List<String> expected = new ArrayList<String>(draws);
        int count = measures[0];
        int firstGap = positions.get(1) - positions.get(0);
        positions.clear();
        draws.clear();
        nextFrame();
        w.render(g, Style.ARCTIC, false);
        assertEquals(expected, draws);
        assertEquals(count + expected.size(), measures[0]);
        assertEquals("X: :" + slot.labelColor, draws.get(0));
        assertEquals("4  :" + slot.textColor, draws.get(1));
        positions.clear();
        glyphWidth[0] = 12; // A resource-pack font reload, without a GUI-scale change.
        w.render(g, Style.ARCTIC, false);
        assertEquals(2 * firstGap, positions.get(1) - positions.get(0));
        scale[0] = 2f;
        w.render(g, Style.ARCTIC, false);
        assertTrue(measures[0] > count);
    }
}
