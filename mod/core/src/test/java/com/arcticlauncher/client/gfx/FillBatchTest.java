package com.arcticlauncher.client.gfx;

import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class FillBatchTest {
    @Test void roundedPanelKeepsEveryRectangleAndClosesOneBatch() {
        boolean original = Draw.fancy;
        Draw.fancy = false;
        try {
            List<String> calls = new ArrayList<String>();
            Gfx g = (Gfx) Proxy.newProxyInstance(Gfx.class.getClassLoader(), new Class<?>[] {Gfx.class}, (p, m, a) -> {
                if (m.getName().equals("fill")) calls.add(java.util.Arrays.toString(a));
                else calls.add(m.getName());
                return null;
            });
            Draw.round(g, 0, 0, 58, 16, 2, 0x6f000000);
            assertEquals("beginFills", calls.get(0));
            assertEquals("endFills", calls.get(6));
            assertEquals("[2, 0, 56, 1, 1862270976]", calls.get(1));
            assertEquals("[2, 15, 56, 16, 1862270976]", calls.get(2));
            assertEquals("[1, 1, 57, 2, 1862270976]", calls.get(3));
            assertEquals("[1, 14, 57, 15, 1862270976]", calls.get(4));
            assertEquals("[0, 2, 58, 14, 1862270976]", calls.get(5));
        } finally { Draw.fancy = original; }
    }
    @Test void failedFillStillClosesBatch() {
        boolean original = Draw.fancy;
        Draw.fancy = false;
        List<String> calls = new ArrayList<String>();
        try {
            Gfx g = (Gfx) Proxy.newProxyInstance(Gfx.class.getClassLoader(), new Class<?>[] {Gfx.class}, (p, m, a) -> {
                calls.add(m.getName());
                if (m.getName().equals("fill")) throw new IllegalStateException("test");
                return null;
            });
            assertThrows(IllegalStateException.class, () -> Draw.outline(g, 0, 0, 58, 16, 2, -1));
            assertEquals("endFills", calls.get(calls.size() - 1));
        } finally { Draw.fancy = original; }
    }
}
