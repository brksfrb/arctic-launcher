package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.*;
import java.io.*;
import java.util.*;
import java.util.zip.*;

/** Sampled native render poses, separate from tick-integrated cape physics.
 * Interpolation defines a recorded curve; it does not attest native animation fidelity. */
final class WingHistory {
    static final String KIND="bedrock_wing_samples_v1";
    private static final long MAGIC=0x41524357494e0001L;
    private static final int MAX_BLOCK=2*1024*1024;
    private static final class Frame {
        double time;long tick;float rate;
        final SortedMap<UUID,WingSample> actors=new TreeMap<UUID,WingSample>();
        boolean same(Frame other) {
            if(tick!=other.tick||Float.floatToIntBits(rate)!=Float.floatToIntBits(other.rate)||!actors.keySet().equals(other.actors.keySet()))return false;
            for(UUID id:actors.keySet())if(!actors.get(id).same(other.actors.get(id)))return false;return true;
        }
    }
    static final class Writer {
        private final AppearanceHistory history;
        private final List<Frame> block=new ArrayList<Frame>();
        private Frame last;
        Writer(AppearanceHistory history){this.history=history;}
        void observe(WorldFrame world,double time) throws IOException {
            try {
                CapeHistory.clock(world);Frame frame=new Frame();frame.time=time;
                frame.tick=world.nativeTiming.get("ticks").getAsLong();frame.rate=world.nativeTiming.get("ticksPerSecond").getAsFloat();
                timing(frame);
                for(WorldFrame.Player player:world.players)if(player.modelObserved()&&player.wingMatrices!=null&&player.partMatrices!=null
                        &&Boolean.TRUE.equals(player.elytraEquipped)) {
                    frame.actors.put(UUID.fromString(player.id),new WingSample(player.wingMatrices,player.partMatrices[Rig.BODY]));
                }
                if(last!=null) {
                    if(time<last.time||frame.tick<last.tick)throw new IOException("wing recording clock moved backwards");
                    if(time==last.time) {
                        if(!last.same(frame))throw new IOException("wing pose changed without a recorded clock advance");
                        return;
                    }
                }
                if(block.size()==64)flush(time);
                block.add(frame);last=frame;
            } catch(IllegalArgumentException invalid) {throw new IOException("invalid native wing history input",invalid);}
        }
        void finish() throws IOException {if(!block.isEmpty())flush(block.get(block.size()-1).time+1);}
        private void flush(double end) throws IOException {
            // All-empty blocks remain gaps, including after wings disappear.
            // Adjacent chunk boundaries cannot bridge a discarded interval.
            boolean present=false;for(Frame frame:block)if(!frame.actors.isEmpty()) {present=true;break;}
            if(present) {
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();
                try(DataOutputStream out=new DataOutputStream(bytes)) {
                    out.writeLong(MAGIC);out.writeInt(block.size());
                    for(Frame frame:block) {
                        out.writeDouble(frame.time);out.writeLong(frame.tick);out.writeFloat(frame.rate);out.writeShort(frame.actors.size());
                        for(Map.Entry<UUID,WingSample> entry:frame.actors.entrySet()) {
                            out.writeLong(entry.getKey().getMostSignificantBits());out.writeLong(entry.getKey().getLeastSignificantBits());entry.getValue().write(out);
                        }
                    }
                }
                if(bytes.size()>MAX_BLOCK)throw new IOException("wing block exceeds 2 MiB");
                history.appendTrack(KIND,bytes.toByteArray(),block.get(0).time,end,block.size());
            }
            block.clear();
        }
    }
    private static void timing(Frame frame) throws IOException {
        if(!Double.isFinite(frame.time)||frame.time<0||frame.time>86_400_000||frame.tick<0||frame.tick>1_000_000_000_000L
                ||!Float.isFinite(frame.rate)||frame.rate<1||frame.rate>1000)throw new IOException("invalid wing sample timing");
    }
    private static final class Chunk {String hash;double start,end;int frames;}
    static final class Reader {
        private final File source;private final String manifest;
        private final List<Chunk> chunks=new ArrayList<Chunk>();
        private final LinkedHashMap<Integer,List<Frame>> cache=new LinkedHashMap<Integer,List<Frame>>(2,.75f,true);
        Reader(File source,AppearanceAssets.Archive archive,JsonArray index) throws IOException {
            this.source=source;manifest=archive.manifestHash;
            if(index.size()<1||index.size()>4096)throw new IOException("invalid wing track index length");
            Frame previous=null;double lastEnd=-1;
            for(JsonElement value:index) {
                JsonObject descriptor=value.getAsJsonObject();Chunk chunk=new Chunk();
                if(descriptor.entrySet().size()!=4)throw new IOException("invalid wing chunk descriptor");
                chunk.hash=descriptor.get("hash").getAsString();chunk.start=number(descriptor,"startMs");chunk.end=number(descriptor,"endMs");
                double count=number(descriptor,"frames");chunk.frames=(int)count;
                if(count!=chunk.frames||chunk.frames<1||chunk.frames>64||chunk.start<0||chunk.end>86_401_000||chunk.end<=chunk.start
                        ||chunk.start<lastEnd)throw new IOException("invalid wing chunk time range");
                List<Frame> decoded=decode(archive,chunk);
                for(Frame frame:decoded) {
                    if(previous!=null&&(frame.time<=previous.time||frame.tick<previous.tick))throw new IOException("wing samples overlap or run backwards");
                    previous=frame;
                }
                chunks.add(chunk);lastEnd=chunk.end;
            }
        }
        private static double number(JsonObject object,String key) throws IOException {
            JsonElement value=object.get(key);
            if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isNumber()||!Double.isFinite(value.getAsDouble()))throw new IOException("invalid wing track number");
            return value.getAsDouble();
        }
        private static List<Frame> decode(AppearanceAssets.Archive archive,Chunk chunk) throws IOException {
            byte[] compressed=archive.read("asset",chunk.hash);ByteArrayOutputStream raw=new ByteArrayOutputStream();
            Inflater inflater=new Inflater();ByteArrayInputStream source=new ByteArrayInputStream(compressed);
            try(InflaterInputStream input=new InflaterInputStream(source,inflater)) {
                byte[] buffer=new byte[16384];int n;
                while((n=input.read(buffer))!=-1) {if(raw.size()+n>MAX_BLOCK)throw new IOException("inflated wing block exceeds 2 MiB");raw.write(buffer,0,n);}
                if(!inflater.finished()||inflater.getRemaining()!=0||source.available()!=0)throw new IOException("wing block has a truncated stream or trailing data");
            } finally {inflater.end();}
            List<Frame> frames=new ArrayList<Frame>();
            try(DataInputStream input=new DataInputStream(new ByteArrayInputStream(raw.toByteArray()))) {
                if(input.readLong()!=MAGIC||input.readInt()!=chunk.frames)throw new IOException("invalid wing block header");
                for(int i=0;i<chunk.frames;i++) {
                    Frame frame=new Frame();frame.time=input.readDouble();frame.tick=input.readLong();frame.rate=input.readFloat();timing(frame);
                    int actors=input.readUnsignedShort();if(actors>128)throw new IOException("wing sample exceeds actor budget");
                    if(!frames.isEmpty()&&(frame.time<=frames.get(frames.size()-1).time||frame.tick<frames.get(frames.size()-1).tick))throw new IOException("invalid wing sample order");
                    for(int actor=0;actor<actors;actor++) {
                        UUID id=new UUID(input.readLong(),input.readLong());
                        if(frame.actors.put(id,WingSample.read(input))!=null)throw new IOException("duplicate wing actor");
                    }
                    frames.add(frame);
                }
                if(input.read()!=-1||Double.doubleToLongBits(frames.get(0).time)!=Double.doubleToLongBits(chunk.start)
                        ||frames.get(frames.size()-1).time>=chunk.end)throw new IOException("wing chunk differs from its index");
            } catch(IllegalArgumentException invalid) {throw new IOException("invalid recorded wing matrices",invalid);}
            return frames;
        }
        private List<Frame> block(int index) throws IOException {
            List<Frame> value=cache.get(index);if(value!=null)return value;
            try(AppearanceAssets.Archive archive=new AppearanceAssets.Archive(source)) {
                if(!manifest.equals(archive.manifestHash))throw new IOException("wing history source changed after mount");
                value=decode(archive,chunks.get(index));
            }
            if(cache.size()>=2)cache.remove(cache.keySet().iterator().next());cache.put(index,value);return value;
        }
        WingSample sample(UUID actor,double time) throws IOException {
            if(!Double.isFinite(time)||time<0||time>86_400_000)return null;
            int low=0,high=chunks.size()-1,index=-1;
            while(low<=high) {int middle=(low+high)>>>1;if(chunks.get(middle).start<=time) {index=middle;low=middle+1;}else high=middle-1;}
            if(index<0||time>=chunks.get(index).end)return null;
            List<Frame> frames=block(index);Frame left=null,right=null;
            for(Frame frame:frames) {if(frame.time>time) {right=frame;break;}left=frame;}
            if(left==null)return null;WingSample a=left.actors.get(actor);if(a==null)return null;
            if(time==left.time)return new WingSample(a.wings,a.body);
            if(right==null&&index+1<chunks.size()&&chunks.get(index).end==chunks.get(index+1).start)right=block(index+1).get(0);
            // No extrapolation, interpolation across a missing actor/tick, or
            // blending across a long unobserved interval such as minimization.
            if(right==null||right.tick-left.tick>1||right.time-left.time>2*Math.max(1000d/left.rate,1000d/right.rate)+.001)return null;
            WingSample b=right.actors.get(actor);if(b==null)return null;
            return WingSample.between(a,b,(time-left.time)/(right.time-left.time));
        }
    }
}
