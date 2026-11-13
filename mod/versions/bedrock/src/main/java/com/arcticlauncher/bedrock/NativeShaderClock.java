package com.arcticlauncher.bedrock;

import com.google.gson.*;

/** Owned CPU clock observation. Parsing this is not evidence of native
 * provenance, shader upload, frame rendering or deterministic video. */
final class NativeShaderClock {
    final String session;
    final long observation,worldSequence,worldScope,tick;
    final double elapsedMs,worldPhase;
    final float partial,ticksPerSecond;
    final boolean worldSkipped;
    final Remainder total,delta,world;
    final NativePresentationClock.Domain presentationDomain;
    private final float[] rendererClock;
    private final String encoded;

    static final class Remainder {
        final double input,nativeResult,effectiveResult;
        final boolean arcticOverride;
        Remainder(JsonObject value) {
            if(value.entrySet().size()!=4)throw invalid();
            input=number(value,"input",0,1_000_000_000);
            nativeResult=number(value,"nativeResult",0,3600);
            effectiveResult=number(value,"effectiveResult",0,3600);
            arcticOverride=flag(value,"arcticOverride");
            if(nativeResult>=3600||effectiveResult>=3600||!arcticOverride&&!bits(nativeResult,effectiveResult))throw invalid();
        }
    }
    private NativeShaderClock(JsonObject value,String expectedSession) {
        long schema=whole(value,"schema",1,2);
        session=text(value,"session");
        if(value.entrySet().size()!=(schema==1?19:20)
                ||!"native_frame_fmod".equals(text(value,"source"))||!expectedSession.equals(session))throw invalid();
        presentationDomain=schema==1?null:new NativePresentationClock.Domain(object(value.get("presentationClockDomain")));
        observation=whole(value,"observation",1,Long.MAX_VALUE);
        worldSequence=whole(value,"worldSequence",1,Long.MAX_VALUE);
        worldScope=whole(value,"worldScope",1,Long.MAX_VALUE);
        tick=whole(value,"ticks",0,1_000_000_000_000L);
        elapsedMs=number(value,"elapsedMs",0,1_000_000_000_000_000.);
        partial=(float)number(value,"partial",0,1);
        ticksPerSecond=(float)number(value,"ticksPerSecond",1,1000);
        if(partial>=1)throw invalid();
        total=new Remainder(object(value.get("total")));
        delta=new Remainder(object(value.get("delta")));
        worldPhase=number(value,"worldPhase",0,3600);
        worldSkipped=flag(value,"worldSkipped");
        JsonElement worldValue=value.get("world");
        if(worldValue==null)throw invalid();
        world=worldValue.isJsonNull()?null:new Remainder(object(worldValue));
        if(worldPhase>=3600||worldSkipped!=(world==null)
                ||world!=null&&(world.arcticOverride||!bits(world.effectiveResult,worldPhase))
                ||!flag(value,"nativeWorldStoreMatched")||flag(value,"shaderUploadVerified")
                ||flag(value,"deterministicVideoReady"))throw invalid();
        rendererClock=clock(value.get("rendererClock"));
        encoded=value.toString();
    }
    private void match(WorldFrame frame) {
        if(worldSequence!=frame.sequence||frame.nativeTiming==null
                ||tick!=whole(frame.nativeTiming,"ticks",0,1_000_000_000_000L)
                ||!bits(elapsedMs,number(frame.nativeTiming,"elapsedMs",0,1_000_000_000_000_000.))
                ||Float.floatToRawIntBits(partial)!=Float.floatToRawIntBits((float)number(frame.nativeTiming,"partial",0,1))
                ||Float.floatToRawIntBits(ticksPerSecond)!=Float.floatToRawIntBits((float)number(frame.nativeTiming,"ticksPerSecond",1,1000)))throw invalid();
        float[] nativeClock=clock(frame.nativeTiming.get("rendererClock"));
        for(int i=0;i<4;i++)if(Float.floatToRawIntBits(rendererClock[i])!=Float.floatToRawIntBits(nativeClock[i]))throw invalid();
    }
    JsonObject value() {return new JsonParser().parse(encoded).getAsJsonObject();}
    static NativeShaderClock archived(JsonObject value,String session) {return new NativeShaderClock(value,session);}
    static NativeShaderClock observe(WorldFrame frame) {
        if(frame.nativeTiming==null)return null;
        JsonElement state=frame.nativeTiming.get("rendererState");
        if(state==null||state.isJsonNull()||!state.isJsonObject())return null;
        JsonElement value=object(state).get("shaderClock");
        if(value==null||value.isJsonNull())return null;
        NativeShaderClock result=new NativeShaderClock(object(value),frame.session);result.match(frame);return result;
    }
    private static float[] clock(JsonElement element) {
        if(element==null||!element.isJsonArray()||element.getAsJsonArray().size()!=4)throw invalid();
        float[] result=new float[4];
        for(int i=0;i<4;i++) {
            JsonElement value=element.getAsJsonArray().get(i);
            if(!numeric(value))throw invalid();
            result[i]=value.getAsFloat();
            if(!Float.isFinite(result[i])||Math.abs(result[i])>1_000_000_000)throw invalid();
        }
        return result;
    }
    private static JsonObject object(JsonElement value) {
        if(value==null||!value.isJsonObject())throw invalid();return value.getAsJsonObject();
    }
    private static boolean numeric(JsonElement value) {
        return value!=null&&value.isJsonPrimitive()&&value.getAsJsonPrimitive().isNumber();
    }
    private static double number(JsonObject value,String name,double min,double max) {
        JsonElement element=value.get(name);if(!numeric(element))throw invalid();
        double result=element.getAsDouble();
        if(!Double.isFinite(result)||result<min||result>max)throw invalid();return result;
    }
    private static long whole(JsonObject value,String name,long min,long max) {
        JsonElement element=value.get(name);if(!numeric(element))throw invalid();
        try {
            long result=new java.math.BigDecimal(element.getAsString()).longValueExact();
            if(result<min||result>max)throw invalid();return result;
        }catch(ArithmeticException|NumberFormatException malformed){throw invalid();}
    }
    private static boolean flag(JsonObject value,String name) {
        JsonElement element=value.get(name);
        if(element==null||!element.isJsonPrimitive()||!element.getAsJsonPrimitive().isBoolean())throw invalid();
        return element.getAsBoolean();
    }
    private static String text(JsonObject value,String name) {
        JsonElement element=value.get(name);
        if(element==null||!element.isJsonPrimitive()||!element.getAsJsonPrimitive().isString())throw invalid();
        return element.getAsString();
    }
    private static boolean bits(double a,double b){return Double.doubleToRawLongBits(a)==Double.doubleToRawLongBits(b);}
    private static IllegalArgumentException invalid(){return new IllegalArgumentException("invalid native shader clock observation");}
}
