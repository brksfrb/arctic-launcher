package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.LocalClock;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.Locale;
import java.util.SimpleTimeZone;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class ClockDisplayTest {
    @Test void localMinuteAndDateBoundariesIncludingClockGoingBackwards() {
        for (int offset : new int[] {10800000, -18000000, 20730000}) {
            for (String pattern : new String[] {"HH:mm", "EEE d MMM"}) {
                SimpleDateFormat format = new SimpleDateFormat(pattern, Locale.ENGLISH);
                format.setTimeZone(new SimpleTimeZone(offset, "fixture"));
                LocalClock.Display display = new LocalClock.Display(format, pattern.equals("HH:mm") ? 60000L : 86400000L);
                long midnight = 1800000000000L / 86400000L * 86400000L - offset;
                for (long delta : new long[] {-60001, -60000, -1, 0, 1, 59999, 60000, 86400000, -1}) {
                    long now = midnight + delta;
                    assertEquals(format.format(new Date(now)), display.at(now));
                }
                String text = display.at(midnight + 1);
                assertSame(text, display.at(midnight + 2));
            }
        }
    }
}
