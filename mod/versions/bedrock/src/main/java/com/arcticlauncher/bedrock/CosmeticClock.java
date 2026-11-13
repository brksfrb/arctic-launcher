package com.arcticlauncher.bedrock;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/** A replay frame's cape/idle time, independent of the new world's tick origin.
 * Only a completed dispatch marker in the same held native sample qualifies.
 * This neither advances physics nor reconstructs recorded Arctic emote state. */
final class CosmeticClock {
    private CosmeticClock() {}
    /** Exact rational identity; conversion to double is for pose math only. */
    static final class Sample {
        final long timeMs;final int remainder,denominator;
        Sample(long timeMs,int remainder,int denominator) {
            if(timeMs<0||timeMs>86_400_000||denominator<1||denominator>1000||remainder<0||remainder>=denominator
                    ||timeMs==86_400_000&&remainder!=0)throw new IllegalArgumentException("invalid replay cosmetic fraction");
            this.timeMs=timeMs;this.remainder=remainder;this.denominator=denominator;
        }
        double exactMillis(){return timeMs+(double)remainder/denominator;}
    }

    static Long replayTime(WorldFrame world) {
        Sample sample=replaySample(world);return sample==null?null:sample.timeMs;
    }
    static Sample replaySample(WorldFrame world) {
        try {
            JsonObject timing=world.nativeTiming;
            if(timing==null)return null;
            long ticks=integer(timing,"ticks",1_000_000_000_000L);
            double partial=timing.get("partial").getAsDouble();
            if(!Double.isFinite(partial)||partial<0||partial>=1)return null;
            JsonObject controls=timing.getAsJsonObject("captureControls");
            JsonObject clock=controls.getAsJsonObject("clock"), packet=controls.getAsJsonObject("packetDispatch");
            if(!world.session.equals(packet.get("session").getAsString())
                    || !"dispatch".equals(packet.get("completion").getAsString())
                    || !"dispatch_returned".equals(packet.get("stage").getAsString())
                    || !truth(packet,"dispatcherReturned") || !truth(packet,"primaryClientThread")
                    || !packet.get("postClientTickCompleted").isJsonPrimitive()
                    || !packet.get("postClientTickCompleted").getAsJsonPrimitive().isBoolean()
                    || packet.get("postClientTickCompleted").getAsBoolean()
                    || !truth(clock,"realTimerControlled")
                    || integer(packet,"dispatcherTick",1_000_000_000_000L)!=ticks
                    || integer(clock,"ticks",1_000_000_000_000L)!=ticks
                    || integer(clock,"freezeSequence",1_000_000_000L)<1
                    || integer(clock,"sequence",1_000_000_000L)<1
                    || integer(packet,"sequence",1_000_000_000L)<1
                    || integer(packet,"generation",1_000_000_000L)<1
                    || integer(packet,"replaySequence",1_000_000_000L)<1
                    || integer(packet,"observationSequence",Long.MAX_VALUE)<1
                    || Float.floatToIntBits(clock.get("partial").getAsFloat())!=Float.floatToIntBits((float)partial)
                    || packet.has("error")&&!packet.get("error").isJsonNull())return null;
            long time=integer(packet,"timeMs",86_400_000);
            if(packet.has("timeRemainder")!=packet.has("timeDenominator"))return null;
            int remainder=packet.has("timeRemainder")?(int)integer(packet,"timeRemainder",999):0;
            int denominator=packet.has("timeDenominator")?(int)integer(packet,"timeDenominator",1000):1;
            if(denominator<1||remainder>=denominator||time==86_400_000&&remainder!=0)return null;
            return new Sample(time,remainder,denominator);
        } catch(RuntimeException invalidReceipt) {return null;}
    }

    private static boolean truth(JsonObject value,String field) {
        JsonElement e=value.get(field);
        return e!=null&&e.isJsonPrimitive()&&e.getAsJsonPrimitive().isBoolean()&&e.getAsBoolean();
    }
    private static long integer(JsonObject value,String field,long max) {
        JsonElement e=value.get(field);
        if(e==null||!e.isJsonPrimitive()||!e.getAsJsonPrimitive().isNumber())throw new IllegalArgumentException("invalid clock integer");
        double number=e.getAsDouble();long integer=e.getAsLong();
        if(!Double.isFinite(number)||integer<0||integer>max||number!=integer)throw new IllegalArgumentException("invalid clock range");
        return integer;
    }
}
