package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.*;
import java.io.*;
import java.util.*;
import java.util.zip.*;

/** Bounded colors, dithering and named CPU inputs for classic cape/wing draws.
 * Unknown and observed absence remain distinct throughout the track. */
final class LayerColorHistory {
    static final String KIND="bedrock_layer_colors_v6",PATHS_KIND="bedrock_layer_colors_v5",CONTEXT_KIND="bedrock_layer_colors_v4",SHADER_KIND="bedrock_layer_colors_v3",DITHERING_KIND="bedrock_layer_colors_v2",LEGACY_KIND="bedrock_layer_colors_v1";
    private static final long MAGIC=0x415243434f4c0006L,PATHS_MAGIC=0x415243434f4c0005L,CONTEXT_MAGIC=0x415243434f4c0004L,SHADER_MAGIC=0x415243434f4c0003L,DITHERING_MAGIC=0x415243434f4c0002L,LEGACY_MAGIC=0x415243434f4c0001L;
    private static final int MAX_BLOCK=1024*1024;
    // 605 bytes per fully populated actor, including UUID, both float/context
    // cohorts and optional path/variation fingerprints. 13*128 plus headers
    // stays below 1 MiB. Older v5/v3-v4 retain 14/16-frame reader limits.
    private static final int BLOCK_FRAMES=13,PATHS_BLOCK_FRAMES=14,LEGACY_SHADER_BLOCK_FRAMES=16;
    private static final class Frame {
        double time;long tick;float rate;
        final SortedMap<UUID,LayerColorSample> actors=new TreeMap<UUID,LayerColorSample>();
        boolean same(Frame other) {
            if(tick!=other.tick||Float.floatToIntBits(rate)!=Float.floatToIntBits(other.rate)||!actors.keySet().equals(other.actors.keySet()))return false;
            for(UUID id:actors.keySet())if(!actors.get(id).same(other.actors.get(id)))return false;return true;
        }
    }
    private static void timing(Frame frame) throws IOException {
        if(!Double.isFinite(frame.time)||frame.time<0||frame.time>86_400_000||frame.tick<0||frame.tick>1_000_000_000_000L
                ||!Float.isFinite(frame.rate)||frame.rate<1||frame.rate>1000)throw new IOException("invalid layer color clock");
    }
    static final class Writer {
        private final AppearanceHistory history;private final List<Frame> block=new ArrayList<Frame>();private Frame last;
        Writer(AppearanceHistory history){this.history=history;}
        void observe(WorldFrame world,double time) throws IOException {
            try {
                CapeHistory.clock(world);Frame frame=new Frame();frame.time=time;
                frame.tick=world.nativeTiming.get("ticks").getAsLong();frame.rate=world.nativeTiming.get("ticksPerSecond").getAsFloat();timing(frame);
                for(WorldFrame.Player player:world.players)if(player.modelObserved())
                    frame.actors.put(UUID.fromString(player.id),LayerColorSample.observe(player));
                if(frame.actors.size()>128)throw new IOException("layer color actor budget exceeded");
                if(last!=null) {
                    if(time<last.time||frame.tick<last.tick)throw new IOException("layer color clock moved backwards");
                    if(time==last.time){if(!frame.same(last))throw new IOException("layer color changed without clock advancement");return;}
                }
                if(block.size()==BLOCK_FRAMES)flush(time);block.add(frame);last=frame;
            }catch(IllegalArgumentException invalid){throw new IOException("invalid observed layer colors",invalid);}
        }
        void finish() throws IOException {if(!block.isEmpty())flush(block.get(block.size()-1).time+1);}
        private void flush(double end) throws IOException {
            boolean present=false;for(Frame frame:block)if(!frame.actors.isEmpty()){present=true;break;}
            if(present) {
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();
                try(DataOutputStream output=new DataOutputStream(bytes)) {
                    output.writeLong(MAGIC);output.writeInt(block.size());
                    for(Frame frame:block) {
                        output.writeDouble(frame.time);output.writeLong(frame.tick);output.writeFloat(frame.rate);output.writeShort(frame.actors.size());
                        for(Map.Entry<UUID,LayerColorSample> actor:frame.actors.entrySet()) {
                            output.writeLong(actor.getKey().getMostSignificantBits());output.writeLong(actor.getKey().getLeastSignificantBits());actor.getValue().write(output,6);
                        }
                    }
                }
                if(bytes.size()>MAX_BLOCK)throw new IOException("layer color block exceeds 1 MiB");
                history.appendTrack(KIND,bytes.toByteArray(),block.get(0).time,end,block.size());
            }
            block.clear();
        }
    }
    private static final class Chunk {String hash;double start,end;int frames;}
    static final class Reader {
        private final File source;private final String manifest;private final List<Chunk> chunks=new ArrayList<Chunk>();
        private final int version;
        private final LinkedHashMap<Integer,List<Frame>> cache=new LinkedHashMap<Integer,List<Frame>>(2,.75f,true);
        Reader(File source,AppearanceAssets.Archive archive,JsonArray index) throws IOException {
            this(source,archive,index,6);
        }
        Reader(File source,AppearanceAssets.Archive archive,JsonArray index,int version) throws IOException {
            if(version<1||version>6)throw new IOException("unsupported layer color track version");
            this.source=source;this.version=version;manifest=archive.manifestHash;
            if(index.size()<1||index.size()>4096)throw new IOException("invalid layer color track length");
            double lastEnd=-1;Frame previous=null;
            for(JsonElement value:index) {
                JsonObject data=value.getAsJsonObject();Chunk chunk=new Chunk();
                if(data.entrySet().size()!=4)throw new IOException("invalid layer color chunk descriptor");
                chunk.hash=data.get("hash").getAsString();chunk.start=number(data,"startMs");chunk.end=number(data,"endMs");
                double count=number(data,"frames");chunk.frames=(int)count;
                if(count!=chunk.frames||chunk.frames<1||chunk.frames>(version>=6?BLOCK_FRAMES:version==5?PATHS_BLOCK_FRAMES:version>=3?LEGACY_SHADER_BLOCK_FRAMES:64)||chunk.start<0||chunk.start<lastEnd||chunk.end<=chunk.start||chunk.end>86_401_000)
                    throw new IOException("invalid layer color chunk range");
                for(Frame frame:decode(archive,chunk)) {
                    if(previous!=null&&(frame.time<=previous.time||frame.tick<previous.tick))throw new IOException("layer color track runs backwards");previous=frame;
                }
                chunks.add(chunk);lastEnd=chunk.end;
            }
        }
        private static double number(JsonObject object,String field) throws IOException {
            JsonElement value=object.get(field);
            if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isNumber()||!Double.isFinite(value.getAsDouble()))throw new IOException("invalid layer color index number");
            return value.getAsDouble();
        }
        private List<Frame> decode(AppearanceAssets.Archive archive,Chunk chunk) throws IOException {
            byte[] compressed=archive.read("asset",chunk.hash);
            if(compressed.length>MAX_BLOCK+65536)throw new IOException("compressed layer colors exceed their bound");
            ByteArrayOutputStream raw=new ByteArrayOutputStream();Inflater inflater=new Inflater();ByteArrayInputStream source=new ByteArrayInputStream(compressed);
            try(InflaterInputStream input=new InflaterInputStream(source,inflater)) {
                byte[] buffer=new byte[16384];int n;
                while((n=input.read(buffer))!=-1){if(raw.size()+n>MAX_BLOCK)throw new IOException("inflated layer colors exceed 1 MiB");raw.write(buffer,0,n);}
                if(!inflater.finished()||inflater.getRemaining()!=0||source.available()!=0)throw new IOException("layer colors have a truncated stream or trailing data");
            }finally{inflater.end();}
            List<Frame> frames=new ArrayList<Frame>();
            try(DataInputStream input=new DataInputStream(new ByteArrayInputStream(raw.toByteArray()))) {
                if(input.readLong()!=(version==6?MAGIC:version==5?PATHS_MAGIC:version==4?CONTEXT_MAGIC:version==3?SHADER_MAGIC:version==2?DITHERING_MAGIC:LEGACY_MAGIC)||input.readInt()!=chunk.frames)throw new IOException("invalid layer color block header");
                for(int i=0;i<chunk.frames;i++) {
                    Frame frame=new Frame();frame.time=input.readDouble();frame.tick=input.readLong();frame.rate=input.readFloat();timing(frame);
                    int actors=input.readUnsignedShort();if(actors>128)throw new IOException("layer color actor budget exceeded");
                    if(!frames.isEmpty()&&(frame.time<=frames.get(frames.size()-1).time||frame.tick<frames.get(frames.size()-1).tick))throw new IOException("layer colors run backwards");
                    for(int actor=0;actor<actors;actor++) {
                        UUID id=new UUID(input.readLong(),input.readLong());if(frame.actors.put(id,LayerColorSample.read(input,version))!=null)throw new IOException("duplicate layer color actor");
                    }
                    frames.add(frame);
                }
                if(input.read()!=-1||Double.doubleToLongBits(frames.get(0).time)!=Double.doubleToLongBits(chunk.start)||frames.get(frames.size()-1).time>=chunk.end)
                    throw new IOException("layer colors differ from their index");
            }catch(IllegalArgumentException invalid){throw new IOException("invalid layer color data",invalid);}
            return frames;
        }
        private List<Frame> block(int index) throws IOException {
            List<Frame> value=cache.get(index);if(value!=null)return value;
            // Evict an old block before decoding the next, keeping the transient
            // raw/decoded peak within this reader's preparation reservation.
            if(cache.size()>=2)cache.remove(cache.keySet().iterator().next());
            try(AppearanceAssets.Archive archive=new AppearanceAssets.Archive(source)) {
                if(!manifest.equals(archive.manifestHash))throw new IOException("layer color source changed after mount");value=decode(archive,chunks.get(index));
            }
            cache.put(index,value);return value;
        }
        boolean hasDithering(){return version>=2;}
        boolean hasShaderInputs(){return version>=3;}
        boolean hasMaterialContext(){return version>=4;}
        boolean hasMaterialPaths(){return version>=5;}
        boolean hasMaterialVariation(){return version>=6;}
        LayerColorSample sample(UUID actor,double time) throws IOException {
            if(!Double.isFinite(time)||time<0||time>86_400_000)return null;
            int low=0,high=chunks.size()-1,index=-1;
            while(low<=high){int mid=(low+high)>>>1;if(chunks.get(mid).start<=time){index=mid;low=mid+1;}else high=mid-1;}
            if(index<0||time>=chunks.get(index).end)return null;
            Frame left=null,right=null;for(Frame frame:block(index)){if(frame.time>time){right=frame;break;}left=frame;}
            if(left==null)return null;LayerColorSample a=left.actors.get(actor);if(a==null)return null;if(time==left.time)return a.copy();
            if(right==null&&index+1<chunks.size()&&chunks.get(index).end==chunks.get(index+1).start)right=block(index+1).get(0);
            if(right==null||right.tick-left.tick>1||right.time-left.time>2*Math.max(1000d/left.rate,1000d/right.rate)+.001)return null;
            LayerColorSample b=right.actors.get(actor);return b==null?null:LayerColorSample.between(a,b,(time-left.time)/(right.time-left.time));
        }
    }
}
