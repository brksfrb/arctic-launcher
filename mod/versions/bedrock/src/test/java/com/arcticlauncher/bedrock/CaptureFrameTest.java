package com.arcticlauncher.bedrock;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class CaptureFrameTest {
    @Test void identityRequiresNativeTimingAndCopiesTheActualWorldCamera() {
        WorldFrame world=new WorldFrame();world.session="current";world.sequence=71;world.clockMs=125;
        world.camera=new double[]{1,2,3,15,10,60};
        assertNull(CaptureFrame.metadata(null,true));
        assertNull(CaptureFrame.metadata(world,true));
        world.nativeTiming=new JsonObject();world.nativeTiming.addProperty("ticks",2);world.nativeTiming.addProperty("partial",.5);
        String first=CaptureFrame.metadata(world,true);
        world.camera[0]=99;
        JsonObject copy=new JsonParser().parse(first).getAsJsonObject();
        assertEquals(1,copy.getAsJsonArray("camera").get(0).getAsDouble());
        assertEquals(71,copy.get("sequence").getAsLong());assertTrue(copy.get("cleanView").getAsBoolean());
        world.nativeTiming.addProperty("partial",Double.NaN);assertNull(CaptureFrame.metadata(world,true));
        world.nativeTiming.addProperty("partial",1);assertNull(CaptureFrame.metadata(world,true));
    }
}
