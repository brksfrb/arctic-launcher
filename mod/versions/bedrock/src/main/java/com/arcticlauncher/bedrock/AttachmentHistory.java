package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.*;
import java.io.*;
import java.util.*;
import java.util.zip.*;

/** Sampled final native attachment palettes, separately indexed from look
 * edits. Geometry/layout or binding changes are discontinuities, not poses to
 * interpolate through. These samples do not certify complete GPU coverage. */
final class AttachmentHistory {
    static final String KIND="bedrock_attachment_samples_v3",CAPE_KIND="bedrock_attachment_samples_v2",LEGACY_KIND="bedrock_attachment_samples_v1";
    private static final long MAGIC=0x4152434154540003L,CAPE_MAGIC=0x4152434154540002L,LEGACY_MAGIC=0x4152434154540001L;
    private static final int MAX_BLOCK=2*1024*1024;
    private static final class Frame {
        double time;long tick;float rate;
        final SortedMap<UUID,AttachmentSample> actors=new TreeMap<UUID,AttachmentSample>();
        boolean same(Frame other) {
            if(tick!=other.tick||Float.floatToIntBits(rate)!=Float.floatToIntBits(other.rate)||!actors.keySet().equals(other.actors.keySet()))return false;
            for(UUID id:actors.keySet())if(!actors.get(id).same(other.actors.get(id)))return false;return true;
        }
    }
    private static void timing(Frame frame) throws IOException {
        if(!Double.isFinite(frame.time)||frame.time<0||frame.time>86_400_000||frame.tick<0||frame.tick>1_000_000_000_000L
                ||!Float.isFinite(frame.rate)||frame.rate<1||frame.rate>1000)throw new IOException("invalid attachment sample clock");
    }
    static final class Writer {
        private final AppearanceHistory history;
        private final List<byte[]> block=new ArrayList<byte[]>();
        private int bytes=12;private Frame last;private double start;private boolean present;
        Writer(AppearanceHistory history){this.history=history;}
        void observe(WorldFrame world,double time) throws IOException {
            try {
                CapeHistory.clock(world);Frame frame=new Frame();frame.time=time;
                frame.tick=world.nativeTiming.get("ticks").getAsLong();frame.rate=world.nativeTiming.get("ticksPerSecond").getAsFloat();timing(frame);
                for(WorldFrame.Player player:world.players)if(player.visible&&player.attachmentPoses!=null&&player.attachmentPoses.complete) {
                    player.attachmentPoses.validate(player);frame.actors.put(UUID.fromString(player.id),player.attachmentPoses.copy());
                }
                if(frame.actors.size()>128)throw new IOException("attachment sample exceeds actor budget");
                if(last!=null) {
                    if(time<last.time||frame.tick<last.tick)throw new IOException("attachment sample clock moved backwards");
                    if(time==last.time){if(!frame.same(last))throw new IOException("attachment pose changed without clock advancement");return;}
                }
                ByteArrayOutputStream encoded=new ByteArrayOutputStream();
                try(DataOutputStream output=new DataOutputStream(encoded)) {
                    output.writeDouble(time);output.writeLong(frame.tick);output.writeFloat(frame.rate);output.writeShort(frame.actors.size());
                    for(Map.Entry<UUID,AttachmentSample> entry:frame.actors.entrySet()) {
                        output.writeLong(entry.getKey().getMostSignificantBits());output.writeLong(entry.getKey().getLeastSignificantBits());entry.getValue().write(output,3);
                    }
                }
                if(encoded.size()>MAX_BLOCK-12)throw new IOException("attachment frame exceeds 2 MiB block budget");
                if(block.size()==64||bytes+encoded.size()>MAX_BLOCK)flush(time);
                if(block.isEmpty())start=time;block.add(encoded.toByteArray());bytes+=encoded.size();last=frame;present|=!frame.actors.isEmpty();
            } catch(RuntimeException invalid) {throw new IOException("invalid observed attachment input",invalid);}
        }
        void finish() throws IOException {if(last!=null)flush(last.time+1);}
        private void flush(double end) throws IOException {
            if(!block.isEmpty()&&present) {
                ByteArrayOutputStream raw=new ByteArrayOutputStream(bytes);
                try(DataOutputStream output=new DataOutputStream(raw)) {output.writeLong(MAGIC);output.writeInt(block.size());for(byte[] frame:block)output.write(frame);}
                history.appendTrack(KIND,raw.toByteArray(),start,end,block.size());
            }
            block.clear();bytes=12;present=false;
        }
    }
    private static final class Chunk {String hash;double start,end;int frames;}
    static final class Reader {
        private final File source;private final String manifest;
        private final int version;
        private final List<Chunk> chunks=new ArrayList<Chunk>();
        private final LinkedHashMap<Integer,List<Frame>> cache=new LinkedHashMap<Integer,List<Frame>>(2,.75f,true);
        Reader(File source,AppearanceAssets.Archive archive,JsonArray index) throws IOException {
            this(source,archive,index,3);
        }
        Reader(File source,AppearanceAssets.Archive archive,JsonArray index,int version) throws IOException {
            if(version<1||version>3)throw new IOException("unsupported attachment history version");
            this.source=source;this.version=version;manifest=archive.manifestHash;
            if(index.size()<1||index.size()>4096)throw new IOException("invalid attachment track length");
            double lastEnd=-1;Frame previous=null;
            for(JsonElement value:index) {
                JsonObject data=value.getAsJsonObject();Chunk chunk=new Chunk();
                if(data.entrySet().size()!=4)throw new IOException("invalid attachment chunk descriptor");
                chunk.hash=data.get("hash").getAsString();chunk.start=number(data,"startMs");chunk.end=number(data,"endMs");
                double count=number(data,"frames");chunk.frames=(int)count;
                if(count!=chunk.frames||chunk.frames<1||chunk.frames>64||chunk.start<0||chunk.start<lastEnd||chunk.end<=chunk.start||chunk.end>86_401_000)
                    throw new IOException("invalid attachment chunk range");
                for(Frame frame:decode(archive,chunk)) {
                    if(previous!=null&&(frame.time<=previous.time||frame.tick<previous.tick))throw new IOException("attachment track runs backwards");previous=frame;
                }
                chunks.add(chunk);lastEnd=chunk.end;
            }
        }
        private static double number(JsonObject value,String key) throws IOException {
            JsonElement field=value.get(key);
            if(field==null||!field.isJsonPrimitive()||!field.getAsJsonPrimitive().isNumber()||!Double.isFinite(field.getAsDouble()))throw new IOException("invalid attachment index number");
            return field.getAsDouble();
        }
        private List<Frame> decode(AppearanceAssets.Archive archive,Chunk chunk) throws IOException {
            byte[] compressed=archive.read("asset",chunk.hash);ByteArrayOutputStream raw=new ByteArrayOutputStream();
            Inflater inflater=new Inflater();ByteArrayInputStream source=new ByteArrayInputStream(compressed);
            try(InflaterInputStream input=new InflaterInputStream(source,inflater)) {
                byte[] buffer=new byte[16384];int n;
                while((n=input.read(buffer))!=-1){if(raw.size()+n>MAX_BLOCK)throw new IOException("inflated attachment block exceeds 2 MiB");raw.write(buffer,0,n);}
                if(!inflater.finished()||inflater.getRemaining()!=0||source.available()!=0)throw new IOException("attachment block is truncated or has trailing data");
            }finally{inflater.end();}
            List<Frame> frames=new ArrayList<Frame>();
            try(DataInputStream input=new DataInputStream(new ByteArrayInputStream(raw.toByteArray()))) {
                if(input.readLong()!=(version==3?MAGIC:version==2?CAPE_MAGIC:LEGACY_MAGIC)||input.readInt()!=chunk.frames)throw new IOException("invalid attachment block header");
                for(int i=0;i<chunk.frames;i++) {
                    Frame frame=new Frame();frame.time=input.readDouble();frame.tick=input.readLong();frame.rate=input.readFloat();timing(frame);
                    if(!frames.isEmpty()&&(frame.time<=frames.get(frames.size()-1).time||frame.tick<frames.get(frames.size()-1).tick))throw new IOException("invalid attachment frame order");
                    int actors=input.readUnsignedShort();if(actors>128)throw new IOException("attachment frame exceeds actor budget");
                    for(int actor=0;actor<actors;actor++) {
                        UUID id=new UUID(input.readLong(),input.readLong());AttachmentSample sample=AttachmentSample.read(input,version);
                        if(!sample.complete||frame.actors.put(id,sample)!=null)throw new IOException("incomplete or duplicate recorded attachment actor");
                    }
                    frames.add(frame);
                }
                if(input.read()!=-1||Double.doubleToLongBits(frames.get(0).time)!=Double.doubleToLongBits(chunk.start)
                        ||frames.get(frames.size()-1).time>=chunk.end)throw new IOException("attachment chunk differs from index");
            }
            return frames;
        }
        private List<Frame> block(int index) throws IOException {
            List<Frame> value=cache.get(index);if(value!=null)return value;
            if(cache.size()>=2)cache.remove(cache.keySet().iterator().next());
            try(AppearanceAssets.Archive archive=new AppearanceAssets.Archive(source)) {
                if(!manifest.equals(archive.manifestHash))throw new IOException("attachment source changed after mount");value=decode(archive,chunks.get(index));
            }
            cache.put(index,value);return value;
        }
        boolean hasNativeCapes(){return version>=2;}
        boolean hasNativeWings(){return version>=3;}
        AttachmentSample sample(UUID actor,double time) throws IOException {
            if(!Double.isFinite(time)||time<0||time>86_400_000)return null;
            int low=0,high=chunks.size()-1,index=-1;
            while(low<=high){int mid=(low+high)>>>1;if(chunks.get(mid).start<=time){index=mid;low=mid+1;}else high=mid-1;}
            if(index<0||time>=chunks.get(index).end)return null;
            Frame left=null,right=null;for(Frame frame:block(index)){if(frame.time>time){right=frame;break;}left=frame;}
            if(left==null)return null;AttachmentSample a=left.actors.get(actor);if(a==null)return null;
            if(time==left.time)return a.copy();
            if(right==null&&index+1<chunks.size()&&chunks.get(index).end==chunks.get(index+1).start)right=block(index+1).get(0);
            if(right==null||right.tick-left.tick>1||right.time-left.time>2*Math.max(1000d/left.rate,1000d/right.rate)+.001)return null;
            AttachmentSample b=right.actors.get(actor);if(b==null||!a.sameLayout(b))return null;
            return AttachmentSample.between(a,b,(time-left.time)/(right.time-left.time));
        }
    }
}
