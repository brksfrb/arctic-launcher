package com.arcticlauncher.bedrock;

import com.google.gson.JsonObject;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;

/** Identity of a world layer actually included in returned/uploaded pixels. */
final class CaptureFrame {
    static String metadata(WorldFrame world, boolean clean) {
        return metadata(world,clean,false);
    }
    static String metadata(WorldFrame world, boolean clean, boolean renderedWorldLayer) {
        return metadata(world,clean,renderedWorldLayer,null);
    }
    static String metadata(WorldFrame world, boolean clean, boolean renderedWorldLayer,JsonObject appearance) {
        if(world==null || world.nativeTiming==null)return null;
        try {
            long ticks=world.nativeTiming.get("ticks").getAsLong();
            double partial=world.nativeTiming.get("partial").getAsDouble();
            if(ticks<0 || ticks>1_000_000_000_000L || !Double.isFinite(partial) || partial<0 || partial>=1)return null;
            JsonObject value=new JsonObject();value.addProperty("schema",1);
            value.addProperty("session",world.session);value.addProperty("sequence",world.sequence);
            value.addProperty("clockMs",world.clockMs);value.addProperty("ticks",ticks);value.addProperty("partial",partial);
            value.addProperty("cleanView",clean);
            if(world.nativeTiming.has("captureControls")) {
                JsonElement controls=new com.google.gson.JsonParser().parse(world.nativeTiming.get("captureControls").toString());
                CosmeticClock.Sample time=world.replayCosmeticSample();
                if(renderedWorldLayer&&time!=null&&controls.isJsonObject()) {
                    JsonObject used=controls.getAsJsonObject();used.addProperty("cosmeticClockMs",time.timeMs);
                    used.addProperty("cosmeticClockRemainder",time.remainder);used.addProperty("cosmeticClockDenominator",time.denominator);
                }
                if(renderedWorldLayer&&appearance!=null&&controls.isJsonObject())controls.getAsJsonObject().add("appearance",appearance);
                value.add("controls",controls);
            }
            JsonArray camera=new JsonArray();for(double v:world.camera)camera.add(new com.google.gson.JsonPrimitive(v));value.add("camera",camera);
            return value.toString();
        } catch(RuntimeException invalidTiming) {return null;}
    }
}
