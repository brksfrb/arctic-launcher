package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.lang.management.ManagementFactory;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

/** CPU bookkeeping only: excludes Minecraft submission and GPU execution. */
class HudCostTest {
    @Test void stableTextWorkload() throws Exception {
        if (!Boolean.getBoolean("arctic.hud.bench")) return;
        Field frame = Hud.class.getDeclaredField("frame");
        frame.setAccessible(true);
        Gfx gfx = (Gfx) Proxy.newProxyInstance(Gfx.class.getClassLoader(), new Class<?>[] {Gfx.class}, (p, m, a) -> {
            if (m.getName().equals("textWidth")) return ((String) a[0]).length() * 6;
            if (m.getReturnType() == int.class) return 720;
            if (m.getReturnType() == float.class) return 2f;
            if (m.getReturnType() == boolean.class) return false;
            return null;
        });
        TextWidget[] widgets = new TextWidget[24];
        for (int i = 0; i < widgets.length; i++) {
            widgets[i] = new TextWidget("fps", "FPS", "", true) {
                protected String value(boolean preview) { return "144"; }
            };
            widgets[i].use(new HudSlot(true));
        }
        com.sun.management.ThreadMXBean bean = (com.sun.management.ThreadMXBean) ManagementFactory.getThreadMXBean();
        long thread = Thread.currentThread().getId();
        for (int pass = 0; pass < 3; pass++) {
            long bytes = bean.getThreadAllocatedBytes(thread), start = System.nanoTime();
            for (int f = 0; f < 20000; f++) {
                frame.setLong(null, f + pass * 20000L);
                for (TextWidget w : widgets) {
                    assertNotNull(w.content(false));
                    w.width(gfx);
                    w.render(gfx, Style.ARCTIC, false);
                }
            }
            System.out.printf("HUD stable 24 widgets pass=%d ns/frame=%.0f bytes/frame=%.0f%n", pass,
                    (System.nanoTime() - start) / 20000.0, (bean.getThreadAllocatedBytes(thread) - bytes) / 20000.0);
        }
    }
}
