package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.*;
import java.util.*;
import java.util.concurrent.atomic.*;

/** Immutable actor-scoped math programs. Native callbacks never access Looks,
 * UI state, files or the game through this service. One client-thread publisher
 * owns leases; callback threads only sample a published immutable state. */
final class EmotePrograms {
    private static final long MAX_BYTES=16L*1024*1024,LEASE_NANOS=250_000_000L;
    private static final long RENDER_PROGRAM_BYTES=1024;
    static final class Seed {
        final String actor;final Object identity;final Animation animation;
        final long origin,min,max;final float seconds;final boolean held,replay;
        final BodyHistory.Curve reference;
        final Double elapsedMillis;
        Seed(String actor,Object identity,Animation animation,long origin,float seconds,boolean held,boolean replay,long min,long max) {
            this(actor,identity,animation,origin,seconds,held,replay,min,max,null);
        }
        Seed(String actor,Object identity,Animation animation,long origin,float seconds,boolean held,boolean replay,long min,long max,BodyHistory.Curve reference) {
            this(actor,identity,animation,origin,seconds,held,replay,min,max,reference,null);
        }
        Seed(String actor,Object identity,Animation animation,long origin,float seconds,boolean held,boolean replay,long min,long max,BodyHistory.Curve reference,Double elapsedMillis) {
            this.actor=actor;this.identity=identity;this.animation=animation;this.origin=origin;this.seconds=seconds;
            this.held=held;this.replay=replay;this.min=min;this.max=max;
            this.reference=reference;this.elapsedMillis=elapsedMillis;
        }
        float phase(double time) {
            return EmotePrograms.phase(held,replay,seconds,elapsedMillis,origin,time);
        }
        boolean same(Seed other) {
            return other!=null&&identity==other.identity&&animation==other.animation&&origin==other.origin
                &&Float.floatToIntBits(seconds)==Float.floatToIntBits(other.seconds)&&held==other.held&&replay==other.replay
                &&Objects.equals(elapsedMillis,other.elapsedMillis)
                &&(reference==null?other.reference==null:reference.same(other.reference));
        }
        void validate() {
            if(actor==null||!UUID.fromString(actor).toString().equals(actor)||identity==null||animation==null
                    ||origin< -86_400_000||origin>1_000_000_000_000L||!Float.isFinite(seconds)||seconds<0||seconds>86400
                    ||min<0||max<min||max>1_000_000_000_000L)throw new IllegalArgumentException("invalid native emote program");
            if(reference!=null&&(!replay||min<reference.firstWholeMillis()||max>reference.lastWholeMillis()))throw new IllegalArgumentException("recorded emote reference differs from its program interval");
            if(elapsedMillis!=null&&(!replay||held||!Double.isFinite(elapsedMillis)||elapsedMillis<0||elapsedMillis>86_400_000
                    ||Float.floatToIntBits((float)(elapsedMillis/1000d))!=Float.floatToIntBits(seconds)))throw new IllegalArgumentException("invalid recorded emote millisecond phase");
        }
    }
    private static final class Program {
        final String token;final Seed seed;
        Program(String token,Seed seed){this.token=token;this.seed=seed;}
    }
    /** Read-only animation identity for an already acknowledged buffered draw.
     * It cannot authorize callbacks and retains neither actor playback state nor
     * recorded body curves. Those remain in the separately leased State. */
    private static final class RenderProgram {
        final String actor;final Animation animation;final long origin,min,max;
        final float seconds;final boolean held,replay,reference;
        final Double elapsedMillis;
        final double referenceStart,referenceEnd;
        RenderProgram(Seed seed) {
            actor=seed.actor;animation=seed.animation;origin=seed.origin;min=seed.min;max=seed.max;
            seconds=seed.seconds;held=seed.held;replay=seed.replay;reference=seed.reference!=null;
            elapsedMillis=seed.elapsedMillis;
            referenceStart=reference?seed.reference.start:0;referenceEnd=reference?seed.reference.end:0;
        }
        boolean matches(WorldFrame.EmotePose pose,String actor,double time) {
            return this.actor.equals(actor)&&observedAt(pose,time)&&(long)time>=min&&(long)time<=max
                &&(!reference||time>=referenceStart&&time<=referenceEnd)&&!animation.finished(pose.seconds)
                &&Float.floatToIntBits(phase(held,replay,seconds,elapsedMillis,origin,time))==Float.floatToIntBits(pose.seconds);
        }
    }
    private static final class RenderAnimation {
        final long bytes;int references;
        RenderAnimation(Animation animation){bytes=animation.retainedBytes();}
    }
    private static final class State {
        final String session;final long at,bytes;final Map<String,Program> actors,tokens;
        final String error;
        State(String session,Map<String,Program> actors,Map<String,Program> tokens,long bytes,String error) {
            this.session=session;this.actors=Collections.unmodifiableMap(actors);this.tokens=Collections.unmodifiableMap(tokens);
            this.bytes=bytes;this.error=error;at=System.nanoTime();
        }
        boolean live(){return System.nanoTime()-at<LEASE_NANOS;}
    }
    private final AtomicReference<State> state=new AtomicReference<State>();
    private final AtomicLong samples=new AtomicLong(),rejected=new AtomicLong();
    private final LinkedHashMap<String,RenderProgram> renderPrograms=new LinkedHashMap<String,RenderProgram>();
    private final IdentityHashMap<Animation,RenderAnimation> renderAnimations=new IdentityHashMap<Animation,RenderAnimation>();
    private String renderSession;private long renderBytes;
    private long generation;
    /** Stop native math authority while retaining bounded buffered-draw data. */
    synchronized void clear(){state.set(null);}
    synchronized void forget() {
        clear();renderPrograms.clear();renderAnimations.clear();renderSession=null;renderBytes=0;
    }
    synchronized Map<String,String> publish(String session,List<Seed> seeds) {
        if(session==null||!session.matches("[A-Za-z0-9_-]{1,64}")||seeds==null||seeds.size()>128)throw new IllegalArgumentException("invalid native emote program publication");
        if(!session.equals(renderSession)){forget();renderSession=session;}
        State old=state.get();Map<String,Program> actors=new HashMap<String,Program>(),tokens=new HashMap<String,Program>();
        Map<String,String> result=new HashMap<String,String>();Set<String> seenActors=new HashSet<String>();
        Set<Animation> animations=Collections.newSetFromMap(new IdentityHashMap<Animation,Boolean>());
        long bytes=256;String error=null;
        for(Seed seed:seeds) {
            seed.validate();if(!seenActors.add(seed.actor))throw new IllegalArgumentException("duplicate native emote actor");
            long weight=RENDER_PROGRAM_BYTES+(animations.contains(seed.animation)?0:seed.animation.retainedBytes())
                +(seed.reference==null?0:seed.reference.retainedBytes());
            if(bytes+weight>MAX_BYTES) {error="native emote programs exceed their retained data budget";continue;}
            Program previous=old!=null&&session.equals(old.session)?old.actors.get(seed.actor):null;
            String token;
            if(previous!=null&&seed.same(previous.seed))token=previous.token;
            else {
                if(generation==Long.MAX_VALUE)throw new IllegalStateException("native emote token space exhausted");
                token=String.format(Locale.ROOT,"%064x",++generation);
            }
            Program program=new Program(token,seed);actors.put(seed.actor,program);tokens.put(token,program);
            result.put(seed.actor,token);animations.add(seed.animation);bytes+=weight;
        }
        state.set(new State(session,actors,tokens,bytes,error));
        // Touch every current program before retiring old ones. The current
        // state's own budget admits all its animations into this render cache.
        for(Program program:actors.values())remember(program);
        while(renderPrograms.size()>512||renderBytes>MAX_BYTES) {
            String token=renderPrograms.keySet().iterator().next();retire(renderPrograms.remove(token));
        }
        return result;
    }
    private void remember(Program program) {
        RenderProgram old=renderPrograms.remove(program.token);if(old!=null)retire(old);
        RenderProgram next=new RenderProgram(program.seed);renderPrograms.put(program.token,next);renderBytes+=RENDER_PROGRAM_BYTES;
        RenderAnimation held=renderAnimations.get(next.animation);
        if(held==null){held=new RenderAnimation(next.animation);renderAnimations.put(next.animation,held);renderBytes+=held.bytes;}
        held.references++;
    }
    private void retire(RenderProgram program) {
        renderBytes-=RENDER_PROGRAM_BYTES;RenderAnimation held=renderAnimations.get(program.animation);
        if(--held.references==0){renderAnimations.remove(program.animation);renderBytes-=held.bytes;}
    }
    synchronized Animation acknowledged(WorldFrame.EmotePose pose,String session,String actor,double time) {
        if(!Objects.equals(renderSession,session)||pose==null)return null;
        RenderProgram program=renderPrograms.get(pose.token);
        return program!=null&&program.matches(pose,actor,time)?program.animation:null;
    }
    /** Returns {sample clock ms, phase seconds, six row-major 3x4 parts}.
     * Replay math retains its rational sub-ms phase in the double clock value;
     * live clocks and native whole-ms identities remain integral. */
    double[] sample(String token,String session,String actor,String timing,float[] baseline) {
        try {
            State current=state.get();Program program=program(current,token,session,actor);
            if(program==null||timing==null||timing.length()>16*1024||baseline==null||baseline.length!=72)return reject();
            JsonObject nativeTiming=new JsonParser().parse(timing).getAsJsonObject();
            integer(nativeTiming,"ticks",1_000_000_000_000L);
            double elapsed=number(nativeTiming,"elapsedMs");number(nativeTiming,"partial");number(nativeTiming,"ticksPerSecond");
            // JSON's shortest f32 spelling must round back through float before
            // the native double arithmetic, especially on integer-ms boundaries.
            double partial=nativeTiming.get("partial").getAsFloat(),rate=nativeTiming.get("ticksPerSecond").getAsFloat();
            if(elapsed<0||elapsed>1_000_000_000_000L||!Double.isFinite(partial)||!Double.isFinite(rate)||partial<0||partial>=1||rate<1||rate>1000)return reject();
            WorldFrame clock=new WorldFrame();clock.session=session;clock.nativeTiming=nativeTiming;
            CosmeticClock.Sample replay=CosmeticClock.replaySample(clock);
            if(program.seed.replay!=(replay!=null))return reject();
            long time=replay==null?(long)(elapsed+partial/rate*1000):replay.timeMs;
            double sampleTime=replay==null?time:replay.exactMillis();
            if(time<program.seed.min||time>program.seed.max)return reject();
            float seconds=program.seed.phase(sampleTime);
            if(!Float.isFinite(seconds)||seconds<0||seconds>86400||program.seed.animation.finished(seconds))return reject();
            float[][] parts=new float[6][12];
            for(int i=0;i<6;i++) {System.arraycopy(baseline,i*12,parts[i],0,12);WorldFrame.rigid(parts[i]);}
            if(program.seed.reference!=null) {
                BodySample recorded=program.seed.reference.sample(sampleTime);if(recorded==null)return reject();
                parts=recorded.copyParts();
            }
            float[][] posed=Rig.poseMatrices(program.seed.animation,seconds,parts);
            double[] result=new double[74];result[0]=sampleTime;result[1]=seconds;
            for(int i=0;i<6;i++) {WorldFrame.rigid(posed[i]);for(int j=0;j<12;j++)result[2+i*12+j]=posed[i][j];}
            // A concurrent stop, event switch, world reset or expired publisher
            // invalidates this callback even if its input was initially valid.
            State latest=state.get();Program live=program(latest,token,session,actor);
            if(live==null||time<live.seed.min||time>live.seed.max)return reject();
            samples.incrementAndGet();return result;
        } catch(RuntimeException|StackOverflowError invalid) {return reject();}
    }
    private double[] reject(){rejected.incrementAndGet();return null;}
    private static Program program(State state,String token,String session,String actor) {
        if(state==null||!state.live()||!state.session.equals(session)||token==null||actor==null)return null;
        Program program=state.tokens.get(token);return program!=null&&program.seed.actor.equals(actor)?program:null;
    }
    boolean matches(WorldFrame.EmotePose pose,String session,String actor,double sampleTime,Animation animation) {
        if(!observedAt(pose,sampleTime))return false;
        long time=(long)sampleTime;
        State current=state.get();Program program=program(current,pose.token,session,actor);
        return program!=null&&program.seed.animation==animation&&pose.timeMs==time&&time>=program.seed.min&&time<=program.seed.max
            &&(program.seed.reference==null||sampleTime>=program.seed.reference.start&&sampleTime<=program.seed.reference.end)
            &&!animation.finished(pose.seconds)
            &&Float.floatToIntBits(program.seed.phase(sampleTime))==Float.floatToIntBits(pose.seconds);
    }
    synchronized JsonObject status() {
        State current=state.get();JsonObject value=new JsonObject();
        value.addProperty("programs",current==null?0:current.actors.size());value.addProperty("retainedBytes",current==null?0:current.bytes);
        value.addProperty("publisherLive",current!=null&&current.live());value.addProperty("error",current==null?null:current.error);
        int recorded=0;if(current!=null)for(Program program:current.actors.values())if(program.seed.reference!=null)recorded++;
        value.addProperty("recordedBodyPrograms",recorded);
        value.addProperty("bufferedRenderPrograms",renderPrograms.size());value.addProperty("bufferedRenderBytes",renderBytes);
        value.addProperty("samples",samples.get());value.addProperty("rejected",rejected.get());
        value.addProperty("nativeEquipmentPoseApplied",false);return value;
    }
    static boolean observedAt(WorldFrame.EmotePose pose,double time) {
        return pose!=null&&Double.isFinite(time)&&time>=0&&time<=1_000_000_000_000L&&pose.timeMs==(long)time
            &&(pose.sampleTimeMs==null?time==(long)time:Double.doubleToLongBits(time)==Double.doubleToLongBits(pose.sampleTimeMs));
    }
    private static float phase(boolean held,boolean replay,float seconds,Double elapsedMillis,long origin,double time) {
        if(held)return seconds;
        if(replay&&elapsedMillis!=null)return (float)((elapsedMillis+(time-origin))/1000d);
        return replay?(float)(seconds+(time-origin)/1000d):(float)Math.max(0,(time-origin)/1000d);
    }
    private static double number(JsonObject object,String field) {
        JsonElement value=object.get(field);
        if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isNumber()||!Double.isFinite(value.getAsDouble()))throw new IllegalArgumentException("invalid native emote clock");
        return value.getAsDouble();
    }
    private static long integer(JsonObject object,String field,long max) {
        double value=number(object,field);long integer=object.get(field).getAsLong();
        if(integer<0||integer>max||value!=integer)throw new IllegalArgumentException("invalid native emote clock integer");return integer;
    }
}
