package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.*;
import java.io.*;
import java.util.*;
import java.util.zip.InflaterInputStream;
import java.util.zip.Inflater;

/** Completed-tick cape/Java wing endpoints, independently indexed from logical look
 * changes. Missing ticks/actors remain gaps; playback never invents history. */
final class CapeHistory {
    static final String KIND="bedrock_cape_ticks_v2",LEGACY_KIND="bedrock_cape_ticks_v1";
    private static final long MAGIC=0x4152434341500002L,LEGACY_MAGIC=0x4152434341500001L;
    private static final int MAX_BLOCK=2*1024*1024;
    private static final class Frame {
        long tick;double start;float rate;
        final SortedMap<UUID,CapePhysics.Snapshot> actors=new TreeMap<UUID,CapePhysics.Snapshot>();
        double end() {return start+1000d/rate;}
    }
    static double clock(WorldFrame world) throws IOException {
        try {
            JsonObject timing=world.nativeTiming;double elapsed=timing.get("elapsedMs").getAsDouble();
            float rate=timing.get("ticksPerSecond").getAsFloat(),partial=timing.get("partial").getAsFloat();
            if(!Double.isFinite(elapsed)||elapsed<0||elapsed>1e12||!Float.isFinite(rate)||rate<1||rate>1000
                    ||!Float.isFinite(partial)||partial<0||partial>=1)throw new IOException("invalid native cape clock");
            return elapsed+(double)partial/rate*1000;
        } catch(RuntimeException missing) {throw new IOException("cape history needs native tick timing",missing);}
    }
    static final class Writer {
        private final AppearanceHistory history;
        private final double anchorElapsed,anchorPhase;private final long origin;
        private Frame current;
        private final List<Frame> block=new ArrayList<Frame>();
        Writer(AppearanceHistory history,WorldFrame world,long origin) throws IOException {
            this.history=history;this.origin=origin;clock(world);
            anchorElapsed=world.nativeTiming.get("elapsedMs").getAsDouble();anchorPhase=phase(world);
        }
        private double phase(WorldFrame world) {
            return (double)world.nativeTiming.get("partial").getAsFloat()/world.nativeTiming.get("ticksPerSecond").getAsFloat()*1000;
        }
        double elapsed(WorldFrame world) throws IOException {
            clock(world);
            // Subtract the large integral clock before adding the small phase.
            // An hours-old clock must not round away a tiny recorded fraction.
            return (world.nativeTiming.get("elapsedMs").getAsDouble()-anchorElapsed)+(phase(world)-anchorPhase);
        }
        void observe(WorldFrame world) throws IOException {
            try {
                clock(world);JsonObject timing=world.nativeTiming;
                long tick=timing.get("ticks").getAsLong();float rate=timing.get("ticksPerSecond").getAsFloat();
                double start=origin+(timing.get("elapsedMs").getAsDouble()-anchorElapsed)-anchorPhase;
                if(tick<0||tick>1_000_000_000_000L||!Double.isFinite(start)||start< -1000||start>86_400_000)throw new IOException("invalid cape tick origin");
                if(current==null||current.tick!=tick) {
                    if(current!=null) {
                        if(tick<current.tick||start<current.end()-0.000001)throw new IOException("cape recording clock moved backwards or overlapped");
                        block.add(current);if(block.size()==64)flush();
                    }
                    current=new Frame();current.tick=tick;current.start=start;current.rate=rate;
                } else if(Double.doubleToLongBits(start)!=Double.doubleToLongBits(current.start)||Float.floatToIntBits(rate)!=Float.floatToIntBits(current.rate))
                    throw new IOException("cape timing changed without a completed tick");
                for(WorldFrame.Player player:world.players) {
                    UUID id=UUID.fromString(player.id);
                    if(player.capeState==null) {
                        if(current.actors.containsKey(id))throw new IOException("cape endpoints disappeared without a completed tick");
                        continue;
                    }
                    CapePhysics.Snapshot state=player.capeState;state.validate();
                    if(state.tick!=tick)throw new IOException("cape state belongs to a different completed tick");
                    if(player.capeMotion!=null&&!state.sample(timing.get("partial").getAsFloat()).same(player.capeMotion))
                        throw new IOException("cape endpoints do not reproduce the observed native motion sample");
                    CapePhysics.Snapshot previous=current.actors.get(id);
                    if(previous!=null&&!previous.same(state))throw new IOException("cape endpoints changed without a completed tick");
                    if(previous==null&&current.actors.size()>=128)throw new IOException("cape tick exceeds actor budget");
                    current.actors.put(id,state);
                }
            } catch(RuntimeException invalid) {throw new IOException("invalid native cape history input",invalid);}
        }
        void finish() throws IOException {if(current!=null) {block.add(current);current=null;}flush();}
        private void flush() throws IOException {
            if(block.isEmpty())return;
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();
            try(DataOutputStream out=new DataOutputStream(bytes)) {
                out.writeLong(MAGIC);out.writeInt(block.size());
                for(Frame frame:block) {
                    out.writeLong(frame.tick);out.writeDouble(frame.start);out.writeFloat(frame.rate);out.writeShort(frame.actors.size());
                    for(Map.Entry<UUID,CapePhysics.Snapshot> entry:frame.actors.entrySet()) {
                        out.writeLong(entry.getKey().getMostSignificantBits());out.writeLong(entry.getKey().getLeastSignificantBits());entry.getValue().write(out,true);
                    }
                }
            }
            if(bytes.size()>MAX_BLOCK)throw new IOException("cape block exceeds its byte budget");
            history.appendTrack(KIND,bytes.toByteArray(),block.get(0).start,block.get(block.size()-1).end(),block.size());block.clear();
        }
    }
    private static final class Chunk {String hash;double start,end;int frames;}
    static final class Reader {
        private final File source;private final String manifest;
        private final int version;
        private final List<Chunk> chunks=new ArrayList<Chunk>();
        private int cached=-1;private List<Frame> cache;
        Reader(File source,AppearanceAssets.Archive archive,JsonArray index) throws IOException {
            this(source,archive,index,2);
        }
        Reader(File source,AppearanceAssets.Archive archive,JsonArray index,int version) throws IOException {
            if(version<1||version>2)throw new IOException("unsupported cape tick version");
            this.source=source;manifest=archive.manifestHash;this.version=version;
            if(index.size()<1||index.size()>4096)throw new IOException("invalid cape track index length");
            long lastTick=-1;double lastEnd=-1000;
            for(JsonElement value:index) {
                JsonObject descriptor=value.getAsJsonObject();Chunk chunk=new Chunk();
                if(descriptor.entrySet().size()!=4)throw new IOException("invalid cape chunk descriptor");
                chunk.hash=descriptor.get("hash").getAsString();chunk.start=number(descriptor,"startMs");chunk.end=number(descriptor,"endMs");
                double count=number(descriptor,"frames");chunk.frames=(int)count;
                if(count!=chunk.frames||chunk.frames<1||chunk.frames>64||chunk.start< -1000||chunk.end>86_401_000||chunk.end<=chunk.start||chunk.start<lastEnd-0.000001)
                    throw new IOException("invalid cape chunk time range");
                List<Frame> decoded=decode(archive,chunk);
                for(Frame frame:decoded) {
                    if(frame.tick<=lastTick||frame.start<lastEnd-0.000001)throw new IOException("cape track ticks overlap or run backwards");
                    lastTick=frame.tick;lastEnd=frame.end();
                }
                chunks.add(chunk);
            }
        }
        private static double number(JsonObject object,String key) throws IOException {
            JsonElement value=object.get(key);
            if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isNumber()||!Double.isFinite(value.getAsDouble()))throw new IOException("invalid cape track number");
            return value.getAsDouble();
        }
        private List<Frame> decode(AppearanceAssets.Archive archive,Chunk chunk) throws IOException {
            byte[] compressed=archive.read("asset",chunk.hash);ByteArrayOutputStream raw=new ByteArrayOutputStream();
            Inflater inflater=new Inflater();ByteArrayInputStream source=new ByteArrayInputStream(compressed);
            try(InflaterInputStream input=new InflaterInputStream(source,inflater)) {
                byte[] buffer=new byte[16384];int n;
                while((n=input.read(buffer))!=-1) {if(raw.size()+n>MAX_BLOCK)throw new IOException("inflated cape block exceeds 2 MiB");raw.write(buffer,0,n);}
                if(!inflater.finished()||inflater.getRemaining()!=0||source.available()!=0)throw new IOException("cape block has a truncated stream or trailing data");
            } finally {inflater.end();}
            List<Frame> frames=new ArrayList<Frame>();
            try(DataInputStream input=new DataInputStream(new ByteArrayInputStream(raw.toByteArray()))) {
                if(input.readLong()!=(version==2?MAGIC:LEGACY_MAGIC)||input.readInt()!=chunk.frames)throw new IOException("invalid cape block header");
                for(int i=0;i<chunk.frames;i++) {
                    Frame frame=new Frame();frame.tick=input.readLong();frame.start=input.readDouble();frame.rate=input.readFloat();int count=input.readUnsignedShort();
                    if(frame.tick<0||frame.tick>1_000_000_000_000L||!Double.isFinite(frame.start)||frame.start< -1000||frame.start>86_400_000
                            ||!Float.isFinite(frame.rate)||frame.rate<1||frame.rate>1000||count>128)throw new IOException("invalid cape frame timing or actor count");
                    if(!frames.isEmpty()&&(frame.tick<=frames.get(frames.size()-1).tick||frame.start<frames.get(frames.size()-1).end()-0.000001))throw new IOException("overlapping cape ticks");
                    for(int actor=0;actor<count;actor++) {
                        UUID id=new UUID(input.readLong(),input.readLong());CapePhysics.Snapshot state=CapePhysics.Snapshot.read(input,frame.tick,version>=2);
                        if(frame.actors.put(id,state)!=null)throw new IOException("duplicate cape actor");
                    }
                    frames.add(frame);
                }
                if(input.read()!=-1||Double.doubleToLongBits(frames.get(0).start)!=Double.doubleToLongBits(chunk.start)
                        ||Double.doubleToLongBits(frames.get(frames.size()-1).end())!=Double.doubleToLongBits(chunk.end))throw new IOException("cape chunk differs from its index");
            } catch(IllegalArgumentException invalid) {throw new IOException("invalid recorded cape state",invalid);}
            return frames;
        }
        boolean hasWingPhysics(){return version>=2;}
        private static final class Sample {
            final CapePhysics.Snapshot state;final float partial;
            Sample(CapePhysics.Snapshot state,float partial){this.state=state;this.partial=partial;}
        }
        private Sample at(UUID actor,double time) throws IOException {
            if(!Double.isFinite(time)||time<0||time>86_400_000)return null;
            int low=0,high=chunks.size()-1,index=-1;
            while(low<=high) {int middle=(low+high)>>>1;if(chunks.get(middle).start<=time) {index=middle;low=middle+1;}else high=middle-1;}
            if(index<0||time>=chunks.get(index).end)return null;
            if(index!=cached)try(AppearanceAssets.Archive archive=new AppearanceAssets.Archive(source)) {
                if(!manifest.equals(archive.manifestHash))throw new IOException("cape history source changed after mount");
                List<Frame> decoded=decode(archive,chunks.get(index));cache=decoded;cached=index;
            }
            Frame selected=null;for(Frame frame:cache) {if(frame.start>time)break;selected=frame;}
            if(selected==null||time>=selected.end())return null;
            CapePhysics.Snapshot state=selected.actors.get(actor);if(state==null)return null;
            float partial=(float)((time-selected.start)*selected.rate/1000);
            return partial<0||partial>=1?null:new Sample(state,partial);
        }
        CapePhysics.Motion sample(UUID actor,double time) throws IOException {
            Sample selected=at(actor,time);return selected==null?null:selected.state.sample(selected.partial);
        }
        JavaWingPhysics.Motion wing(UUID actor,double time) throws IOException {
            Sample selected=at(actor,time);
            return selected==null||selected.state.wingState==null?null:selected.state.wingState.sample(selected.partial);
        }
    }
}
