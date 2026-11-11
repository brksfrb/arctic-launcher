package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.*;
import java.io.*;
import java.util.*;
import java.util.zip.*;

/** Owned material/lightmap observations. A missing sample is a recorded gap,
 * not permission to borrow the live viewer's shader inputs. */
final class EntityMaterialHistory {
    static final String KIND="bedrock_entity_material_v1";
    private static final long MAGIC=0x4152434d41540001L;
    private static final int MAX_BLOCK=512*1024,BLOCK_FRAMES=64;

    private static final class Sample {
        final JavaEntityShader shader;
        final String dimension;
        final boolean relative;
        final double[] camera;
        final float[] projection;
        Sample(JavaEntityShader shader,String dimension,boolean relative,double[] camera,float[] projection) {
            if(shader==null||dimension==null||dimension.length()>64||camera==null||camera.length!=6||projection==null||projection.length!=16)
                throw new IllegalArgumentException("invalid material observation context");
            for(double value:camera)if(!Double.isFinite(value)||Math.abs(value)>32_000_000)throw new IllegalArgumentException("invalid material camera");
            for(float value:projection)if(!Float.isFinite(value)||Math.abs(value)>1e8f)throw new IllegalArgumentException("invalid material projection");
            if(camera[5]<1||camera[5]>179)throw new IllegalArgumentException("invalid material camera FOV");
            this.shader=shader;this.dimension=dimension;this.relative=relative;
            this.camera=camera.clone();this.projection=projection.clone();
        }
        static Sample observe(WorldFrame world) {
            JavaEntityShader shader=world.entityShader();if(shader==null)return null;
            if(world.cameraRelative==null)throw new IllegalArgumentException("material capture requires a declared position space");
            return new Sample(shader,world.dimension,world.cameraRelative,world.camera,world.viewProjection);
        }
        boolean same(Sample other) {
            return other!=null&&shader.same(other.shader)&&sameContext(other);
        }
        private boolean sameContext(Sample other) {
            return dimension.equals(other.dimension)&&relative==other.relative
                &&Arrays.equals(camera,other.camera)&&Arrays.equals(projection,other.projection);
        }
        private boolean transferable() {return relative&&shader.cameraRelativeWorldFog();}
        boolean stableWith(Sample other) {
            return other!=null&&shader.same(other.shader)&&dimension.equals(other.dimension)&&relative==other.relative
                &&(transferable()&&other.transferable()||sameContext(other));
        }
        JavaEntityShader forWorld(WorldFrame world) {
            if(world==null||world.cameraRelative==null||relative!=world.cameraRelative||!dimension.equals(world.dimension))return null;
            // Position is already relative to the selected replay camera.
            // Identity fog therefore follows camera edits without a guessed
            // transform. Arbitrary affine profiles retain their source camera.
            if(transferable()||Arrays.equals(camera,world.camera)&&Arrays.equals(projection,world.viewProjection))return shader;
            return null;
        }
        void write(DataOutput output) throws IOException {
            output.writeUTF(dimension);output.writeByte(relative?1:0);
            for(double value:camera)output.writeDouble(value);for(float value:projection)output.writeFloat(value);
            shader.write(output);
        }
        static Sample read(DataInput input) throws IOException {
            String dimension=input.readUTF();int relative=input.readUnsignedByte();
            if(relative>1)throw new IOException("invalid recorded material position space");
            double[] camera=new double[6];float[] projection=new float[16];
            for(int i=0;i<6;i++)camera[i]=input.readDouble();for(int i=0;i<16;i++)projection[i]=input.readFloat();
            try{return new Sample(JavaEntityShader.read(input),dimension,relative==1,camera,projection);}
            catch(IllegalArgumentException invalid){throw new IOException("invalid recorded material context",invalid);}
        }
    }
    private static final class Frame {
        double time;long tick;float rate;Sample sample;
        boolean same(Frame other) {
            return tick==other.tick&&Float.floatToIntBits(rate)==Float.floatToIntBits(other.rate)
                &&(sample==null?other.sample==null:sample.same(other.sample));
        }
    }
    private static void timing(Frame frame) throws IOException {
        if(!Double.isFinite(frame.time)||frame.time<0||frame.time>86_400_000||frame.tick<0||frame.tick>1_000_000_000_000L
                ||!Float.isFinite(frame.rate)||frame.rate<1||frame.rate>1000)throw new IOException("invalid material observation clock");
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
                timing(frame);frame.sample=Sample.observe(world);
                if(last!=null) {
                    if(time<last.time||frame.tick<last.tick)throw new IOException("material recording clock moved backwards");
                    if(time==last.time){if(!frame.same(last))throw new IOException("material changed without recorded clock advancement");return;}
                }
                // A fully populated frame is below 4600 bytes, including the
                // bounded UTF dimension, context and 1058 material floats.
                if(block.size()==BLOCK_FRAMES)flush(time);block.add(frame);last=frame;
            }catch(IllegalArgumentException invalid){throw new IOException("invalid material history input",invalid);}
        }
        void finish() throws IOException {if(!block.isEmpty())flush(block.get(block.size()-1).time+1);}
        private void flush(double end) throws IOException {
            boolean present=false;for(Frame frame:block)if(frame.sample!=null){present=true;break;}
            if(present) {
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();
                try(DataOutputStream output=new DataOutputStream(bytes)) {
                    output.writeLong(MAGIC);output.writeInt(block.size());
                    for(Frame frame:block) {
                        output.writeDouble(frame.time);output.writeLong(frame.tick);output.writeFloat(frame.rate);output.writeBoolean(frame.sample!=null);
                        if(frame.sample!=null)frame.sample.write(output);
                    }
                }
                if(bytes.size()>MAX_BLOCK)throw new IOException("material history block exceeds 512 KiB");
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
            if(index.size()<1||index.size()>4096)throw new IOException("invalid material track index length");
            double lastEnd=-1;Frame previous=null;
            for(JsonElement value:index) {
                JsonObject data=value.getAsJsonObject();Chunk chunk=new Chunk();
                if(data.entrySet().size()!=4)throw new IOException("invalid material chunk descriptor");
                chunk.hash=data.get("hash").getAsString();chunk.start=number(data,"startMs");chunk.end=number(data,"endMs");
                double count=number(data,"frames");chunk.frames=(int)count;
                if(count!=chunk.frames||chunk.frames<1||chunk.frames>BLOCK_FRAMES||chunk.start<0||chunk.start<lastEnd
                        ||chunk.end<=chunk.start||chunk.end>86_401_000)throw new IOException("invalid material chunk range");
                for(Frame frame:decode(archive,chunk)) {
                    if(previous!=null&&(frame.time<=previous.time||frame.tick<previous.tick))throw new IOException("material track runs backwards");
                    previous=frame;
                }
                chunks.add(chunk);lastEnd=chunk.end;
            }
        }
        private static double number(JsonObject object,String field) throws IOException {
            JsonElement value=object.get(field);
            if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isNumber()||!Double.isFinite(value.getAsDouble()))
                throw new IOException("invalid material track number");
            return value.getAsDouble();
        }
        private static List<Frame> decode(AppearanceAssets.Archive archive,Chunk chunk) throws IOException {
            byte[] compressed=archive.read("asset",chunk.hash);
            if(compressed.length>MAX_BLOCK+65536)throw new IOException("compressed material block exceeds bound");
            ByteArrayOutputStream raw=new ByteArrayOutputStream();Inflater inflater=new Inflater();ByteArrayInputStream source=new ByteArrayInputStream(compressed);
            try(InflaterInputStream input=new InflaterInputStream(source,inflater)) {
                byte[] buffer=new byte[16384];int count;
                while((count=input.read(buffer))!=-1){if(raw.size()+count>MAX_BLOCK)throw new IOException("inflated material block exceeds 512 KiB");raw.write(buffer,0,count);}
                if(!inflater.finished()||inflater.getRemaining()!=0||source.available()!=0)throw new IOException("material block is truncated or has trailing compressed data");
            }finally{inflater.end();}
            List<Frame> frames=new ArrayList<Frame>();
            try(DataInputStream input=new DataInputStream(new ByteArrayInputStream(raw.toByteArray()))) {
                if(input.readLong()!=MAGIC||input.readInt()!=chunk.frames)throw new IOException("invalid material block header");
                for(int i=0;i<chunk.frames;i++) {
                    Frame frame=new Frame();frame.time=input.readDouble();frame.tick=input.readLong();frame.rate=input.readFloat();timing(frame);
                    int present=input.readUnsignedByte();if(present>1)throw new IOException("invalid material presence flag");
                    frame.sample=present==1?Sample.read(input):null;
                    if(!frames.isEmpty()&&(frame.time<=frames.get(frames.size()-1).time||frame.tick<frames.get(frames.size()-1).tick))throw new IOException("invalid material sample order");
                    frames.add(frame);
                }
                if(input.read()!=-1||Double.doubleToLongBits(frames.get(0).time)!=Double.doubleToLongBits(chunk.start)
                        ||frames.get(frames.size()-1).time>=chunk.end)throw new IOException("material chunk differs from index");
            }
            return frames;
        }
        private List<Frame> block(int index) throws IOException {
            List<Frame> value=cache.get(index);if(value!=null)return value;
            try(AppearanceAssets.Archive archive=new AppearanceAssets.Archive(source)) {
                if(!manifest.equals(archive.manifestHash))throw new IOException("material source changed after mount");value=decode(archive,chunks.get(index));
            }
            if(cache.size()>=2)cache.remove(cache.keySet().iterator().next());cache.put(index,value);return value;
        }
        JavaEntityShader sample(double time,WorldFrame world) throws IOException {
            if(!Double.isFinite(time)||time<0||time>86_400_000)return null;
            int low=0,high=chunks.size()-1,index=-1;
            while(low<=high){int middle=(low+high)>>>1;if(chunks.get(middle).start<=time){index=middle;low=middle+1;}else high=middle-1;}
            if(index<0||time>=chunks.get(index).end)return null;
            Frame left=null,right=null;
            for(Frame frame:block(index)){if(frame.time>time){right=frame;break;}left=frame;}
            if(left==null||left.sample==null)return null;
            if(time==left.time)return left.sample.forWorld(world);
            if(right==null&&index+1<chunks.size()&&chunks.get(index).end==chunks.get(index+1).start)right=block(index+1).get(0);
            if(right==null||right.tick-left.tick>1||right.time-left.time>2*Math.max(1000d/left.rate,1000d/right.rate)+.001
                    ||!left.sample.stableWith(right.sample))return null;
            return left.sample.forWorld(world);
        }
    }
}
