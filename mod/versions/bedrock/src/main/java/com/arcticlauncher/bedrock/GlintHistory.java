package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.*;
import java.io.*;
import java.util.*;
import java.util.zip.*;

/** Recorded selected glint inputs. Missing/changing observations stay gaps;
 * no live clock/settings, invented phase curve or camera rebase is substituted. */
final class GlintHistory {
    static final String KIND="bedrock_glint_samples_v1";
    private static final long MAGIC=0x415243474c540001L;
    private static final int MAX_BLOCK=2*1024*1024;
    private static final class Context {
        final String dimension;final boolean relative;
        final double[] camera;final float[] projection,equipment;
        Context(String dimension,boolean relative,double[] camera,float[] projection,float[] equipment) {
            if(dimension==null||dimension.length()>64||camera==null||camera.length!=6||projection==null||projection.length!=16
                    ||equipment==null||equipment.length!=16)throw new IllegalArgumentException("invalid glint camera context");
            for(double value:camera)if(!Double.isFinite(value)||Math.abs(value)>32_000_000)throw new IllegalArgumentException("invalid glint camera");
            for(float[] matrix:new float[][]{projection,equipment})for(float value:matrix)
                if(!Float.isFinite(value)||Math.abs(value)>1e8f)throw new IllegalArgumentException("invalid glint projection");
            if(camera[5]<1||camera[5]>179)throw new IllegalArgumentException("invalid glint camera FOV");
            this.dimension=dimension;this.relative=relative;this.camera=camera.clone();
            this.projection=projection.clone();this.equipment=equipment.clone();
        }
        static Context observe(WorldFrame world) {
            if(world.cameraRelative==null)throw new IllegalArgumentException("glint history needs a declared position space");
            return new Context(world.dimension,world.cameraRelative,world.camera,world.viewProjection,world.copyEquipmentProjection());
        }
        boolean same(Context other) {return other!=null&&dimension.equals(other.dimension)&&relative==other.relative
            &&Arrays.equals(camera,other.camera)&&Arrays.equals(projection,other.projection)&&Arrays.equals(equipment,other.equipment);}
        boolean matches(WorldFrame world) {return world!=null&&world.cameraRelative!=null&&relative==world.cameraRelative
            &&dimension.equals(world.dimension)&&Arrays.equals(camera,world.camera)&&Arrays.equals(projection,world.viewProjection)
            &&Arrays.equals(equipment,world.copyEquipmentProjection());}
        void write(DataOutput out) throws IOException {
            out.writeUTF(dimension);out.writeByte(relative?1:0);
            for(double value:camera)out.writeDouble(value);for(float value:projection)out.writeFloat(value);for(float value:equipment)out.writeFloat(value);
        }
        static Context read(DataInput in) throws IOException {
            String dimension=in.readUTF();int relative=in.readUnsignedByte();
            if(relative>1)throw new IOException("invalid recorded glint position space");
            double[] camera=new double[6];float[] projection=new float[16],equipment=new float[16];
            for(int i=0;i<6;i++)camera[i]=in.readDouble();for(int i=0;i<16;i++)projection[i]=in.readFloat();for(int i=0;i<16;i++)equipment[i]=in.readFloat();
            return new Context(dimension,relative==1,camera,projection,equipment);
        }
    }
    private static final class Frame {
        double time;long tick;float rate;Context context;
        final SortedMap<UUID,GlintSample> actors=new TreeMap<UUID,GlintSample>();
        boolean same(Frame other) {
            if(tick!=other.tick||Float.floatToIntBits(rate)!=Float.floatToIntBits(other.rate)||!actors.keySet().equals(other.actors.keySet())
                    ||(context==null?other.context!=null:!context.same(other.context)))return false;
            for(UUID id:actors.keySet())if(!actors.get(id).same(other.actors.get(id)))return false;return true;
        }
        GlintSample forWorld(GlintSample sample,WorldFrame world) {
            return sample==null||sample.selected&&(context==null||!context.matches(world))?null:sample.copy();
        }
    }
    private static void timing(Frame frame) throws IOException {
        if(!Double.isFinite(frame.time)||frame.time<0||frame.time>86_400_000||frame.tick<0||frame.tick>1_000_000_000_000L
                ||!Float.isFinite(frame.rate)||frame.rate<1||frame.rate>1000)throw new IOException("invalid glint sample clock");
    }
    static final class Writer {
        private final AppearanceHistory history;private final List<Frame> block=new ArrayList<Frame>();private Frame last;
        Writer(AppearanceHistory history){this.history=history;}
        void observe(WorldFrame world,double time) throws IOException {
            try {
                CapeHistory.clock(world);Frame frame=new Frame();frame.time=time;
                frame.tick=world.nativeTiming.get("ticks").getAsLong();frame.rate=world.nativeTiming.get("ticksPerSecond").getAsFloat();timing(frame);
                for(WorldFrame.Player player:world.players)if(player.modelObserved()&&player.wingMatrices!=null&&player.partMatrices!=null
                        &&Boolean.TRUE.equals(player.elytraEquipped)&&player.wingGlint!=null) {
                    frame.actors.put(UUID.fromString(player.id),player.wingGlint.copy());
                    if(player.wingGlint.selected&&frame.context==null)frame.context=Context.observe(world);
                }
                if(last!=null) {
                    if(time<last.time||frame.tick<last.tick)throw new IOException("glint history clock moved backwards");
                    if(time==last.time){if(!frame.same(last))throw new IOException("glint changed without a recorded clock advance");return;}
                }
                if(block.size()==64)flush(time);block.add(frame);last=frame;
            }catch(IllegalArgumentException invalid){throw new IOException("invalid glint observation",invalid);}
        }
        void finish() throws IOException {if(!block.isEmpty())flush(block.get(block.size()-1).time+1);}
        private void flush(double end) throws IOException {
            boolean present=false;for(Frame frame:block)if(!frame.actors.isEmpty()){present=true;break;}
            if(present) {
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();
                try(DataOutputStream out=new DataOutputStream(bytes)) {
                    out.writeLong(MAGIC);out.writeInt(block.size());
                    for(Frame frame:block) {
                        out.writeDouble(frame.time);out.writeLong(frame.tick);out.writeFloat(frame.rate);out.writeBoolean(frame.context!=null);
                        if(frame.context!=null)frame.context.write(out);out.writeShort(frame.actors.size());
                        for(Map.Entry<UUID,GlintSample> actor:frame.actors.entrySet()) {
                            out.writeLong(actor.getKey().getMostSignificantBits());out.writeLong(actor.getKey().getLeastSignificantBits());actor.getValue().write(out);
                        }
                    }
                }
                if(bytes.size()>MAX_BLOCK)throw new IOException("glint history block exceeds 2 MiB");
                history.appendTrack(KIND,bytes.toByteArray(),block.get(0).time,end,block.size());
            }
            block.clear();
        }
    }
    private static final class Chunk {String hash;double start,end;int frames;}
    static final class Reader {
        private final File source;private final String manifest;
        private final List<Chunk> chunks=new ArrayList<Chunk>();
        private final LinkedHashMap<Integer,List<Frame>> cache=new LinkedHashMap<Integer,List<Frame>>(2,.75f,true);
        Reader(File source,AppearanceAssets.Archive archive,JsonArray index) throws IOException {
            this.source=source;manifest=archive.manifestHash;
            if(index.size()<1||index.size()>4096)throw new IOException("invalid glint track index length");
            Frame previous=null;double lastEnd=-1;
            for(JsonElement value:index) {
                JsonObject data=value.getAsJsonObject();Chunk chunk=new Chunk();
                if(data.entrySet().size()!=4)throw new IOException("invalid glint chunk descriptor");
                JsonElement hash=data.get("hash");
                if(hash==null||!hash.isJsonPrimitive()||!hash.getAsJsonPrimitive().isString())throw new IOException("invalid glint chunk hash");
                chunk.hash=hash.getAsString();chunk.start=number(data,"startMs");chunk.end=number(data,"endMs");
                double count=number(data,"frames");chunk.frames=(int)count;
                if(!chunk.hash.matches("[0-9a-f]{40}")||count!=chunk.frames||chunk.frames<1||chunk.frames>64||chunk.start<0
                        ||chunk.end>86_401_000||chunk.end<=chunk.start||chunk.start<lastEnd)throw new IOException("invalid glint chunk index");
                for(Frame frame:decode(archive,chunk)) {
                    if(previous!=null&&(frame.time<=previous.time||frame.tick<previous.tick))throw new IOException("glint history runs backwards");
                    previous=frame;
                }
                chunks.add(chunk);lastEnd=chunk.end;
            }
        }
        private static double number(JsonObject input,String key) throws IOException {
            JsonElement value=input.get(key);
            if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isNumber()||!Double.isFinite(value.getAsDouble()))
                throw new IOException("invalid glint index number");
            return value.getAsDouble();
        }
        private static List<Frame> decode(AppearanceAssets.Archive archive,Chunk chunk) throws IOException {
            byte[] compressed=archive.read("asset",chunk.hash);if(compressed.length>MAX_BLOCK+65536)throw new IOException("compressed glint block exceeds bound");
            ByteArrayOutputStream raw=new ByteArrayOutputStream();Inflater inflater=new Inflater();ByteArrayInputStream source=new ByteArrayInputStream(compressed);
            try(InflaterInputStream input=new InflaterInputStream(source,inflater)) {
                byte[] buffer=new byte[16384];int count;
                while((count=input.read(buffer))!=-1){if(raw.size()+count>MAX_BLOCK)throw new IOException("inflated glint block exceeds 2 MiB");raw.write(buffer,0,count);}
                if(!inflater.finished()||inflater.getRemaining()!=0||source.available()!=0)throw new IOException("glint block is truncated or has trailing compressed data");
            }finally{inflater.end();}
            List<Frame> frames=new ArrayList<Frame>();
            try(DataInputStream input=new DataInputStream(new ByteArrayInputStream(raw.toByteArray()))) {
                if(input.readLong()!=MAGIC||input.readInt()!=chunk.frames)throw new IOException("invalid glint block header");
                for(int i=0;i<chunk.frames;i++) {
                    Frame frame=new Frame();frame.time=input.readDouble();frame.tick=input.readLong();frame.rate=input.readFloat();timing(frame);
                    int context=input.readUnsignedByte();if(context>1)throw new IOException("invalid glint context presence");
                    if(context==1)frame.context=Context.read(input);
                    int actors=input.readUnsignedShort();if(actors>128)throw new IOException("glint actor budget exceeded");
                    if(!frames.isEmpty()&&(frame.time<=frames.get(frames.size()-1).time||frame.tick<frames.get(frames.size()-1).tick))throw new IOException("invalid glint sample order");
                    boolean active=false;
                    for(int actor=0;actor<actors;actor++) {
                        UUID id=new UUID(input.readLong(),input.readLong());GlintSample sample=GlintSample.read(input);
                        if(frame.actors.put(id,sample)!=null)throw new IOException("duplicate glint actor");active|=sample.selected;
                    }
                    if(active!=(frame.context!=null))throw new IOException("glint context differs from active sample presence");
                    frames.add(frame);
                }
                if(input.read()!=-1||Double.doubleToLongBits(frames.get(0).time)!=Double.doubleToLongBits(chunk.start)
                        ||frames.get(frames.size()-1).time>=chunk.end)throw new IOException("glint block differs from its index");
            }catch(IllegalArgumentException invalid){throw new IOException("invalid recorded glint context",invalid);}
            return frames;
        }
        private List<Frame> block(int index) throws IOException {
            List<Frame> result=cache.get(index);if(result!=null)return result;
            if(cache.size()>=2)cache.remove(cache.keySet().iterator().next());
            try(AppearanceAssets.Archive archive=new AppearanceAssets.Archive(source)) {
                if(!manifest.equals(archive.manifestHash))throw new IOException("glint history source changed after mount");
                result=decode(archive,chunks.get(index));
            }
            cache.put(index,result);return result;
        }
        GlintSample sample(UUID actor,double time,WorldFrame world) throws IOException {
            if(!Double.isFinite(time)||time<0||time>86_400_000)return null;
            int low=0,high=chunks.size()-1,index=-1;
            while(low<=high){int middle=(low+high)>>>1;if(chunks.get(middle).start<=time){index=middle;low=middle+1;}else high=middle-1;}
            if(index<0||time>=chunks.get(index).end)return null;
            Frame left=null,right=null;
            for(Frame frame:block(index)){if(frame.time>time){right=frame;break;}left=frame;}
            if(left==null)return null;GlintSample a=left.actors.get(actor);if(a==null)return null;
            if(time==left.time)return left.forWorld(a,world);
            if(right==null&&index+1<chunks.size()&&chunks.get(index).end==chunks.get(index+1).start)right=block(index+1).get(0);
            if(right==null||right.tick-left.tick>1||right.time-left.time>2*Math.max(1000d/left.rate,1000d/right.rate)+.001
                    ||!a.same(right.actors.get(actor))||a.selected&&(left.context==null||!left.context.same(right.context)))return null;
            return left.forWorld(a,world);
        }
    }
}
