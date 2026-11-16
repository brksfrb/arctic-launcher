package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.util.Arrays;
import java.util.Map;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class HudLayoutCacheTest {
    static class Counter extends HudWidget {
        int contentCalls, draws, size = 58;
        Counter(String id) { super(id, id, "", true, Column.LEFT); }
        public Object content(boolean preview) { contentCalls++; return size; }
        public int width(Gfx g) { return size; }
        public int height() { return 16; }
        public void render(Gfx g, Style s, boolean preview) { draws++; }
    }
    @Test void stableLayoutReusedAndHiddenWidgetsNeverDraw() throws Exception {
        Field platform = ArcticClient.class.getDeclaredField("platform");
        platform.setAccessible(true);
        Object original = platform.get(null);
        try {
            platform.set(null, Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class},
                    (p, m, a) -> m.getReturnType() == boolean.class ? m.getName().equals("isKeyDown") : null));
            ClientConfig config = new ClientConfig();
            Counter visible = new Counter("visible"), hidden = new Counter("hidden"), off = new Counter("off");
            HudSlot shown = new HudSlot(true), tab = new HudSlot(true), disabled = new HudSlot(false);
            shown.styled = tab.styled = disabled.styled = true;
            tab.hideOnTab = true;
            config.hud.put("visible", shown); config.hud.put("hidden", tab); config.hud.put("off", disabled);
            Hud hud = new Hud(config);
            Field widgets = Hud.class.getDeclaredField("widgets");
            widgets.setAccessible(true);
            widgets.set(hud, Arrays.asList(visible, hidden, off));
            Gfx g = (Gfx) Proxy.newProxyInstance(Gfx.class.getClassLoader(), new Class<?>[] {Gfx.class}, (p, m, a) -> {
                if (m.getName().equals("width")) return 800;
                if (m.getName().equals("height")) return 600;
                if (m.getReturnType() == float.class) return 1f;
                if (m.getReturnType() == boolean.class) return false;
                return null;
            });
            Map<HudWidget, int[]> first = hud.layout(g);
            for (int i = 0; i < 8; i++) assertSame(first, hud.layout(g));
            hud.render(g, Style.ARCTIC, false);
            assertEquals(1, visible.draws);
            assertEquals(0, hidden.draws);
            assertEquals(0, off.draws);
            assertEquals(0, off.contentCalls);
            visible.size++;
            Map<HudWidget, int[]> resized = hud.layout(g);
            assertNotSame(first, resized);
            assertEquals(59, resized.get(visible)[2]);
            shown.enabled = false;
            assertFalse(hud.layout(g).containsKey(visible));
            shown.enabled = true;
            assertTrue(hud.layout(g).containsKey(visible));
            hud.render(g, Style.ARCTIC, true);
            assertEquals(1, hidden.draws);
        } finally { platform.set(null, original); }
    }
}
