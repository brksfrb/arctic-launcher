package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.*;
import java.io.*;
import java.util.*;
import java.util.zip.*;

/** Observed native body references/model roots, independently indexed from appearance edits.
 * Interpolation specifies a sampled curve; it does not certify native animation. */
final class BodyHistory {
    static final String KIND="bedrock_body_samples_v7",VISIBILITY_KIND="bedrock_body_samples_v6",MATERIALS_KIND="bedrock_body_samples_v5",LIGHTS_KIND="bedrock_body_samples_v4",COLORS_KIND="bedrock_body_samples_v3",ROOTS_KIND="bedrock_body_samples_v2",LEGACY_KIND="bedrock_body_samples_v1";
    private static final long MAGIC=0x415243424f440007L,VISIBILITY_MAGIC=0x415243424f440006L,MATERIALS_MAGIC=0x415243424f440005L,LIGHTS_MAGIC=0x415243424f440004L,COLORS_MAGIC=0x415243424f440003L,ROOTS_MAGIC=0x415243424f440002L,LEGACY_MAGIC=0x415243424f440001L;
    private static final int MAX_BLOCK=2*1024*1024;
    private static final class Frame {
        double time;long tick;float rate;
        final SortedMap<UUID,BodySample> actors=new TreeMap<UUID,BodySample>();
        boolean same(Frame other) {
            if(tick!=other.tick||Float.floatToIntBits(rate)!=Float.floatToIntBits(other.rate)||!actors.keySet().equals(other.actors.keySet()))return false;
            for(UUID id:actors.keySet())if(!actors.get(id).same(other.actors.get(id)))return false;return true;
        }
    }
    static final class Writer {
        private final AppearanceHistory history;
        private final List<Frame> block=new ArrayList<Frame>();
        private Frame last;private int bytes=12;
        Writer(AppearanceHistory history){this.history=history;}
        void observe(WorldFrame world,double time) throws IOException {
            try {
                CapeHistory.clock(world);Frame frame=new Frame();frame.time=time;
                frame.tick=world.nativeTiming.get("ticks").getAsLong();frame.rate=world.nativeTiming.get("ticksPerSecond").getAsFloat();timing(frame);
                for(WorldFrame.Player player:world.players)if(player.modelObserved()&&player.partMatrices!=null) {
                    frame.actors.put(UUID.fromString(player.id),new BodySample(player.partMatrices,ModelRootSample.observe(world,player),player.nativeSkinColor,player.packedLight,player.skinMaterial,player.skinVisibility,player.layerVisibility));
                }
                if(frame.actors.size()>128)throw new IOException("body sample exceeds actor budget");
                if(last!=null) {
                    if(time<last.time||frame.tick<last.tick)throw new IOException("body recording clock moved backwards");
                    if(time==last.time) {
                        if(!last.same(frame))throw new IOException("body pose changed without a recorded clock advance");
                        return;
                    }
                }
                int size=22+frame.actors.size()*(16+6*12*4+9*4+3*8+1+4*4+1+4+1+4*4+3+3);
                if(block.size()==64||bytes+size>MAX_BLOCK)flush(time);
                block.add(frame);bytes+=size;last=frame;
            } catch(IllegalArgumentException invalid) {throw new IOException("invalid native body history input",invalid);}
        }
        void finish() throws IOException {if(!block.isEmpty())flush(block.get(block.size()-1).time+1);}
        private void flush(double end) throws IOException {
            boolean present=false;for(Frame frame:block)if(!frame.actors.isEmpty()) {present=true;break;}
            if(present) {
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();
                try(DataOutputStream out=new DataOutputStream(bytes)) {
                    out.writeLong(MAGIC);out.writeInt(block.size());
                    for(Frame frame:block) {
                        out.writeDouble(frame.time);out.writeLong(frame.tick);out.writeFloat(frame.rate);out.writeShort(frame.actors.size());
                        for(Map.Entry<UUID,BodySample> entry:frame.actors.entrySet()) {
                            out.writeLong(entry.getKey().getMostSignificantBits());out.writeLong(entry.getKey().getLeastSignificantBits());entry.getValue().write(out,true,true,true,true,true,true);
                        }
                    }
                }
                if(bytes.size()>MAX_BLOCK)throw new IOException("body block exceeds 2 MiB");
                history.appendTrack(KIND,bytes.toByteArray(),block.get(0).time,end,block.size());
            }
            block.clear();bytes=12;
        }
    }
    private static void timing(Frame frame) throws IOException {
        if(!Double.isFinite(frame.time)||frame.time<0||frame.time>86_400_000||frame.tick<0||frame.tick>1_000_000_000_000L
                ||!Float.isFinite(frame.rate)||frame.rate<1||frame.rate>1000)throw new IOException("invalid body sample timing");
    }
    /** Immutable, file-free interval safe for an attached native callback. */
    static final class Curve {
        final double start,end;private final BodySample left,right;
        Curve(double start,double end,BodySample left,BodySample right) {
            if(!Double.isFinite(start)||!Double.isFinite(end)||start<0||end>86_400_000||end<start
                    ||left==null||right==null||start==end&&!left.same(right))throw new IllegalArgumentException("invalid recorded body interval");
            this.start=start;this.end=end;this.left=left.copy();this.right=right.copy();
        }
        // Native emote receipts retain the whole-ms identity. Fractional
        // membership is checked again when sampling this exact interval.
        long firstWholeMillis(){return (long)Math.floor(start);}
        long lastWholeMillis(){return (long)Math.floor(end);}
        BodySample sample(double time) {
            if(!Double.isFinite(time)||time<start||time>end)return null;
            if(time==start||start==end)return left.copy();
            if(time==end)return right.copy();
            return BodySample.between(left,right,(time-start)/(end-start));
        }
        boolean same(Curve other) {
            return other!=null&&Double.doubleToLongBits(start)==Double.doubleToLongBits(other.start)
                &&Double.doubleToLongBits(end)==Double.doubleToLongBits(other.end)&&left.same(other.left)&&right.same(other.right);
        }
        long retainedBytes(){return 2816;}
    }
    private static final class Chunk {String hash;double start,end;int frames;}
    static final class Reader {
        private final File source;private final String manifest;
        private final int version;
        private final List<Chunk> chunks=new ArrayList<Chunk>();
        private final LinkedHashMap<Integer,List<Frame>> cache=new LinkedHashMap<Integer,List<Frame>>(2,.75f,true);
        Reader(File source,AppearanceAssets.Archive archive,JsonArray index) throws IOException {
            this(source,archive,index,7);
        }
        Reader(File source,AppearanceAssets.Archive archive,JsonArray index,boolean roots) throws IOException {
            this(source,archive,index,roots?2:1);
        }
        Reader(File source,AppearanceAssets.Archive archive,JsonArray index,int version) throws IOException {
            if(version<1||version>7)throw new IOException("unsupported body sample version");
            this.source=source;manifest=archive.manifestHash;this.version=version;
            if(index.size()<1||index.size()>4096)throw new IOException("invalid body track index length");
            Frame previous=null;double lastEnd=-1;
            for(JsonElement value:index) {
                JsonObject descriptor=value.getAsJsonObject();Chunk chunk=new Chunk();
                if(descriptor.entrySet().size()!=4)throw new IOException("invalid body chunk descriptor");
                chunk.hash=descriptor.get("hash").getAsString();chunk.start=number(descriptor,"startMs");chunk.end=number(descriptor,"endMs");
                double count=number(descriptor,"frames");chunk.frames=(int)count;
                if(count!=chunk.frames||chunk.frames<1||chunk.frames>64||chunk.start<0||chunk.end>86_401_000||chunk.end<=chunk.start
                        ||chunk.start<lastEnd)throw new IOException("invalid body chunk time range");
                List<Frame> decoded=decode(archive,chunk,version);
                for(Frame frame:decoded) {
                    if(previous!=null&&(frame.time<=previous.time||frame.tick<previous.tick))throw new IOException("body samples overlap or run backwards");
                    previous=frame;
                }
                chunks.add(chunk);lastEnd=chunk.end;
            }
        }
        private static double number(JsonObject object,String key) throws IOException {
            JsonElement value=object.get(key);
            if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isNumber()||!Double.isFinite(value.getAsDouble()))throw new IOException("invalid body track number");
            return value.getAsDouble();
        }
        boolean hasRoots(){return version>=2;}
        boolean hasColors(){return version>=3;}
        boolean hasPackedLight(){return version>=4;}
        boolean hasSkinMaterials(){return version>=5;}
        boolean hasSkinVisibility(){return version>=6;}
        boolean hasLayerVisibility(){return version>=7;}
        private static List<Frame> decode(AppearanceAssets.Archive archive,Chunk chunk,int version) throws IOException {
            byte[] compressed=archive.read("asset",chunk.hash);ByteArrayOutputStream raw=new ByteArrayOutputStream();
            Inflater inflater=new Inflater();ByteArrayInputStream source=new ByteArrayInputStream(compressed);
            try(InflaterInputStream input=new InflaterInputStream(source,inflater)) {
                byte[] buffer=new byte[16384];int n;
                while((n=input.read(buffer))!=-1) {if(raw.size()+n>MAX_BLOCK)throw new IOException("inflated body block exceeds 2 MiB");raw.write(buffer,0,n);}
                if(!inflater.finished()||inflater.getRemaining()!=0||source.available()!=0)throw new IOException("body block has a truncated stream or trailing data");
            } finally {inflater.end();}
            List<Frame> frames=new ArrayList<Frame>();
            try(DataInputStream input=new DataInputStream(new ByteArrayInputStream(raw.toByteArray()))) {
                if(input.readLong()!=(version==7?MAGIC:version==6?VISIBILITY_MAGIC:version==5?MATERIALS_MAGIC:version==4?LIGHTS_MAGIC:version==3?COLORS_MAGIC:version==2?ROOTS_MAGIC:LEGACY_MAGIC)||input.readInt()!=chunk.frames)throw new IOException("invalid body block header");
                for(int i=0;i<chunk.frames;i++) {
                    Frame frame=new Frame();frame.time=input.readDouble();frame.tick=input.readLong();frame.rate=input.readFloat();timing(frame);
                    int actors=input.readUnsignedShort();if(actors>128)throw new IOException("body sample exceeds actor budget");
                    if(!frames.isEmpty()&&(frame.time<=frames.get(frames.size()-1).time||frame.tick<frames.get(frames.size()-1).tick))throw new IOException("invalid body sample order");
                    for(int actor=0;actor<actors;actor++) {
                        UUID id=new UUID(input.readLong(),input.readLong());if(frame.actors.put(id,BodySample.read(input,version>=2,version>=3,version>=4,version>=5,version>=6,version>=7))!=null)throw new IOException("duplicate body actor");
                    }
                    frames.add(frame);
                }
                if(input.read()!=-1||Double.doubleToLongBits(frames.get(0).time)!=Double.doubleToLongBits(chunk.start)
                        ||frames.get(frames.size()-1).time>=chunk.end)throw new IOException("body chunk differs from its index");
            } catch(IllegalArgumentException invalid) {throw new IOException("invalid recorded body matrices",invalid);}
            return frames;
        }
        private List<Frame> block(int index) throws IOException {
            List<Frame> value=cache.get(index);if(value!=null)return value;
            try(AppearanceAssets.Archive archive=new AppearanceAssets.Archive(source)) {
                if(!manifest.equals(archive.manifestHash))throw new IOException("body history source changed after mount");
                value=decode(archive,chunks.get(index),version);
            }
            if(cache.size()>=2)cache.remove(cache.keySet().iterator().next());cache.put(index,value);return value;
        }
        Curve curve(UUID actor,double time) throws IOException {
            if(!Double.isFinite(time)||time<0||time>86_400_000)return null;
            int low=0,high=chunks.size()-1,index=-1;
            while(low<=high) {int middle=(low+high)>>>1;if(chunks.get(middle).start<=time) {index=middle;low=middle+1;}else high=middle-1;}
            if(index<0||time>=chunks.get(index).end)return null;
            List<Frame> frames=block(index);Frame left=null,right=null;
            for(Frame frame:frames) {if(frame.time>time) {right=frame;break;}left=frame;}
            if(left==null)return null;BodySample a=left.actors.get(actor);if(a==null)return null;
            if(right==null&&index+1<chunks.size()&&chunks.get(index).end==chunks.get(index+1).start)right=block(index+1).get(0);
            // A missing actor, chunk gap, missed tick or long pause is not a
            // license to extrapolate the last pose into the unobserved world.
            if(right!=null&&right.tick-left.tick<=1&&right.time-left.time<=2*Math.max(1000d/left.rate,1000d/right.rate)+.001) {
                BodySample b=right.actors.get(actor);if(b!=null)return new Curve(left.time,right.time,a,b);
            }
            return time==left.time?new Curve(left.time,left.time,a,a):null;
        }
        BodySample sample(UUID actor,double time) throws IOException {Curve curve=curve(actor,time);return curve==null?null:curve.sample(time);}
    }
}
