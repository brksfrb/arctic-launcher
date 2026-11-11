package com.arcticlauncher.bedrock;

import com.google.gson.*;

/** Recording-marker association of presentation time, independent of held
 * simulation ticks. An association is not a verified relay alignment. */
final class NativePresentationClock {
    static final class Domain {
        final String profile;
        final long epochNanoseconds,frequency;
        Domain(JsonObject value) {
            if(value.entrySet().size()!=5||whole(value,"schema",1,1)!=1
                    ||!"qpc_game_epoch".equals(text(value,"source")))throw invalid();
            profile=text(value,"profile");
            if(!profile.matches("[0-9]{1,5}(?:\\.[0-9]{1,5}){2,3}"))throw invalid();
            epochNanoseconds=whole(value,"epochNanoseconds",0,Long.MAX_VALUE);
            frequency=whole(value,"frequency",1,Long.MAX_VALUE);
        }
        boolean same(Domain other) {
            return other!=null&&profile.equals(other.profile)&&epochNanoseconds==other.epochNanoseconds&&frequency==other.frequency;
        }
        JsonObject value() {
            JsonObject value=new JsonObject();value.addProperty("schema",1);value.addProperty("source","qpc_game_epoch");
            value.addProperty("profile",profile);value.addProperty("epochNanoseconds",epochNanoseconds);value.addProperty("frequency",frequency);
            return value;
        }
    }
    private static final class Marker {
        final Domain domain;
        final long nanoseconds;
        final double seconds;
        Marker(JsonObject value) {
            if(value.entrySet().size()!=4||whole(value,"schema",1,1)!=1)throw invalid();
            domain=new Domain(object(value.get("domain")));
            nanoseconds=whole(value,"nanoseconds",0,1_000_000_000_000_000_000L);
            seconds=number(value,"seconds",0,1_000_000_000);
            if(!bits(seconds,(double)nanoseconds/1_000_000_000.))throw invalid();
        }
    }
    /** A single marker-derived affine mapping. Its provenance remains unverified;
     * it is never substituted for the simulation timeline. */
    static final class Mapping {
        final Domain domain;
        final double seconds,timeMs;
        final long observation,worldSequence;
        Mapping(JsonObject source) {
            validateAssociation(source);
            JsonObject data=object(source.get("shaderClockAssociation"));
            domain=new Domain(object(data.get("domain")));
            seconds=number(data,"presentationSeconds",0,1_000_000_000);
            timeMs=number(data,"presentationTimeMs",0,86_400_000);
            observation=whole(data,"observation",1,Long.MAX_VALUE);
            worldSequence=whole(data,"worldSequence",1,Long.MAX_VALUE);
        }
        double time(NativeShaderClock clock) {
            if(!domain.same(clock.presentationDomain)||clock.observation<observation||clock.worldSequence<worldSequence)throw invalid();
            double result=timeMs+(clock.total.input-seconds)*1000.;
            if(!Double.isFinite(result)||clock.total.input<seconds||result<0||result>86_400_000)throw invalid();
            return result;
        }
    }
    static Mapping mapping(JsonObject source) {
        if(source==null)return null;
        validateAssociation(source);
        JsonElement association=source.get("shaderClockAssociation");
        return association==null||association.isJsonNull()?null:new Mapping(source);
    }
    static JsonObject associate(WorldFrame frame,JsonObject anchor,long relayTime) {
        Marker marker=marker(anchor);
        NativeShaderClock clock=frame.shaderClock();
        if(marker==null||clock==null||!marker.domain.same(clock.presentationDomain))return null;
        double elapsed=(clock.total.input-marker.seconds)*1000.;
        double mapped=relayTime+elapsed;
        if(!Double.isFinite(elapsed)||elapsed<0||elapsed>3000||mapped<0||mapped>86_400_000)return null;
        JsonObject result=new JsonObject();result.addProperty("schema",1);result.add("domain",clock.presentationDomain.value());
        result.addProperty("observation",clock.observation);result.addProperty("worldSequence",clock.worldSequence);
        result.addProperty("presentationSeconds",clock.total.input);result.addProperty("presentationTimeMs",mapped);
        result.addProperty("simulationElapsedMs",clock.elapsedMs+(double)clock.partial/clock.ticksPerSecond*1000.);
        result.addProperty("alignmentVerified",false);return result;
    }
    static void validateAssociation(JsonObject source) {
        Marker marker=marker(source);
        JsonElement value=source.get("shaderClockAssociation");
        boolean available=value!=null&&!value.isJsonNull();
        JsonElement declared=source.get("presentationClockMappingAvailable");
        if(declared!=null&&flag(source,"presentationClockMappingAvailable")!=available)throw invalid();
        if(!available)return;
        JsonObject data=object(value);
        if(marker==null||data.entrySet().size()!=8||whole(data,"schema",1,1)!=1
                ||!marker.domain.same(new Domain(object(data.get("domain"))))||flag(data,"alignmentVerified"))throw invalid();
        whole(data,"observation",1,Long.MAX_VALUE);whole(data,"worldSequence",1,Long.MAX_VALUE);
        double seconds=number(data,"presentationSeconds",0,1_000_000_000);
        double elapsed=(seconds-marker.seconds)*1000.;
        double mapped=whole(source,"relayTimeMs",0,86_400_000)+elapsed;
        if(!Double.isFinite(elapsed)||elapsed<0||elapsed>3000
                ||!bits(mapped,number(data,"presentationTimeMs",0,86_400_000)))throw invalid();
        number(data,"simulationElapsedMs",0,1_000_000_000_001_000.);
    }
    private static Marker marker(JsonObject source) {
        JsonElement value=source.get("presentationClock");
        return value==null||value.isJsonNull()?null:new Marker(object(value));
    }
    private static JsonObject object(JsonElement value) {
        if(value==null||!value.isJsonObject())throw invalid();return value.getAsJsonObject();
    }
    private static boolean numeric(JsonElement value) {
        return value!=null&&value.isJsonPrimitive()&&value.getAsJsonPrimitive().isNumber();
    }
    private static double number(JsonObject value,String name,double min,double max) {
        JsonElement element=value.get(name);if(!numeric(element))throw invalid();double result=element.getAsDouble();
        if(!Double.isFinite(result)||result<min||result>max)throw invalid();return result;
    }
    private static long whole(JsonObject value,String name,long min,long max) {
        JsonElement element=value.get(name);if(!numeric(element))throw invalid();
        try {long result=new java.math.BigDecimal(element.getAsString()).longValueExact();if(result<min||result>max)throw invalid();return result;}
        catch(ArithmeticException|NumberFormatException malformed){throw invalid();}
    }
    private static boolean flag(JsonObject value,String name) {
        JsonElement element=value.get(name);
        if(element==null||!element.isJsonPrimitive()||!element.getAsJsonPrimitive().isBoolean())throw invalid();return element.getAsBoolean();
    }
    private static String text(JsonObject value,String name) {
        JsonElement element=value.get(name);
        if(element==null||!element.isJsonPrimitive()||!element.getAsJsonPrimitive().isString())throw invalid();return element.getAsString();
    }
    private static boolean bits(double a,double b){return Double.doubleToRawLongBits(a)==Double.doubleToRawLongBits(b);}
    private static IllegalArgumentException invalid(){return new IllegalArgumentException("invalid native presentation-clock association");}
}
