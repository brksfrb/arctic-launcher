package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.zip.*;

/** Observed CPU shader clocks indexed by presentation time. Simulation tick,
 * partial, elapsed time and actual native remainders remain in each sample.
 * No interpolation, phase restoration or GPU provenance is implied. */
final class ShaderClockHistory {
    static final String KIND="bedrock_shader_clock_v1";
    private static final long MAGIC=0x4152435348430001L;
    private static final int MAX_BLOCK=1024*1024,MAX_SAMPLE=8192;
    private static final class Point {
        final double time;
        final NativeShaderClock clock;
        Point(double time,NativeShaderClock clock){this.time=time;this.clock=clock;}
    }
    private static boolean bits(double a,double b){return Double.doubleToRawLongBits(a)==Double.doubleToRawLongBits(b);}
    private static void ordered(Point before,Point after) throws IOException {
        if(before!=null&&(after.time<before.time||after.clock.total.input<before.clock.total.input
                ||after.clock.observation<before.clock.observation||after.clock.worldSequence<=before.clock.worldSequence
                ||after.clock.tick<before.clock.tick||after.clock.elapsedMs<before.clock.elapsedMs))
            throw new IOException("shader clock observations moved backwards or repeated");
    }
    static final class Writer {
        private final AppearanceHistory history;
        private final NativePresentationClock.Mapping mapping;
        private final String session;
        private final List<Point> block=new ArrayList<Point>();
        private Point last;
        private long observations,missing;
        Writer(AppearanceHistory history,JsonObject source,String session) {
            this.history=history;this.session=session;mapping=NativePresentationClock.mapping(source);
        }
        void observe(WorldFrame world) throws IOException {
            if(mapping==null)return;
            if(!session.equals(world.session))throw new IOException("shader clock recording changed world");
            NativeShaderClock clock=world.shaderClock();
            // Missing observations remain gaps. No held or synthetic sample.
            if(clock==null){missing++;return;}
            try {
                Point point=new Point(mapping.time(clock),clock);
                if(last!=null&&clock.worldSequence==last.clock.worldSequence) {
                    if(bits(point.time,last.time)&&clock.value().equals(last.clock.value()))return;
                    throw new IOException("shader clock observation identity changed contents");
                }
                ordered(last,point);
                if(block.size()==64) {
                    // Equal-time samples are retained; do not relabel them with
                    // invented timestamps to make a chunk fit.
                    if(point.time==last.time) {
                        int split=0;while(split<block.size()&&block.get(split).time<point.time)split++;
                        if(split==0)throw new IOException("shader clock equal-time block budget exhausted");
                        List<Point> tail=new ArrayList<Point>(block.subList(split,block.size()));
                        block.subList(split,block.size()).clear();flush(point.time);block.addAll(tail);
                    } else flush(point.time);
                }
                block.add(point);last=point;observations++;
            }catch(IllegalArgumentException invalid){throw new IOException("shader clock recording lost its presentation domain",invalid);}
        }
        void finish() throws IOException {
            // endMs is an exclusive index bound, not an extra observation.
            if(!block.isEmpty())flush(Math.nextUp(last.time));
        }
        JsonObject status() {
            JsonObject value=new JsonObject();value.addProperty("mappingAvailable",mapping!=null);
            value.addProperty("observations",observations);value.addProperty("missingObservations",missing);
            if(last!=null)value.addProperty("lastPresentationTimeMs",last.time);
            value.addProperty("alignmentVerified",false);value.addProperty("recordedPhaseApplied",false);return value;
        }
        private void flush(double end) throws IOException {
            ByteArrayOutputStream bytes=new ByteArrayOutputStream();
            try(DataOutputStream out=new DataOutputStream(bytes)) {
                out.writeLong(MAGIC);out.writeInt(block.size());
                for(Point point:block) {
                    byte[] encoded=point.clock.value().toString().getBytes(StandardCharsets.UTF_8);
                    if(encoded.length>MAX_SAMPLE)throw new IOException("shader clock sample exceeds 8 KiB");
                    out.writeDouble(point.time);out.writeInt(encoded.length);out.write(encoded);
                }
            }
            if(bytes.size()>MAX_BLOCK)throw new IOException("shader clock block exceeds 1 MiB");
            history.appendTrack(KIND,bytes.toByteArray(),block.get(0).time,end,block.size());block.clear();
        }
    }
    private static final class Chunk {String hash;double start,end;int count;}
    static final class Reader {
        private final File file;
        private final String manifest,session;
        private final NativePresentationClock.Mapping mapping;
        private final List<Chunk> chunks=new ArrayList<Chunk>();
        private double firstTime,lastTime;
        private final LinkedHashMap<Integer,List<Point>> cache=new LinkedHashMap<Integer,List<Point>>(2,.75f,true);
        Reader(File file,AppearanceAssets.Archive archive,JsonArray index,JsonObject source,String session) throws IOException {
            this.file=file;manifest=archive.manifestHash;this.session=session;
            mapping=NativePresentationClock.mapping(source);
            if(mapping==null||index.size()<1||index.size()>4096)throw new IOException("shader clock track needs a bounded index and presentation association");
            Point previous=null;double end=-1;
            for(JsonElement element:index) {
                if(!element.isJsonObject())throw new IOException("invalid shader clock descriptor");
                JsonObject value=element.getAsJsonObject();Chunk chunk=new Chunk();JsonElement hash=value.get("hash");
                if(value.entrySet().size()!=4||hash==null||!hash.isJsonPrimitive()||!hash.getAsJsonPrimitive().isString())
                    throw new IOException("invalid shader clock descriptor");
                chunk.hash=hash.getAsString();chunk.start=number(value,"startMs");chunk.end=number(value,"endMs");
                double count=number(value,"frames");chunk.count=(int)count;
                if(!chunk.hash.matches("[0-9a-f]{40}")||chunk.start<0||chunk.end>86_401_000||chunk.end<=chunk.start
                        ||chunk.start<end||count!=chunk.count||chunk.count<1||chunk.count>64)throw new IOException("invalid shader clock index");
                for(Point point:decode(archive,chunk)) {
                    ordered(previous,point);if(previous==null)firstTime=point.time;previous=point;lastTime=point.time;
                }
                chunks.add(chunk);end=chunk.end;
            }
        }
        private List<Point> decode(AppearanceAssets.Archive archive,Chunk chunk) throws IOException {
            byte[] compressed=archive.read("asset",chunk.hash,MAX_BLOCK+65536);
            ByteArrayOutputStream raw=new ByteArrayOutputStream();Inflater inflater=new Inflater();
            ByteArrayInputStream source=new ByteArrayInputStream(compressed);
            try(InflaterInputStream input=new InflaterInputStream(source,inflater)) {
                byte[] buffer=new byte[16384];int count;
                while((count=input.read(buffer))!=-1) {
                    if(raw.size()+count>MAX_BLOCK)throw new IOException("inflated shader clock block exceeds 1 MiB");
                    raw.write(buffer,0,count);
                }
                if(!inflater.finished()||inflater.getRemaining()!=0||source.available()!=0)throw new IOException("truncated shader clock block or trailing compressed data");
            }finally{inflater.end();}
            List<Point> result=new ArrayList<Point>();
            try(DataInputStream input=new DataInputStream(new ByteArrayInputStream(raw.toByteArray()))) {
                if(input.readLong()!=MAGIC||input.readInt()!=chunk.count)throw new IOException("invalid shader clock block header");
                Point previous=null;
                for(int i=0;i<chunk.count;i++) {
                    double time=input.readDouble();int count=input.readInt();
                    if(count<1||count>MAX_SAMPLE)throw new IOException("invalid shader clock sample size");
                    byte[] encoded=new byte[count];input.readFully(encoded);
                    JsonElement value=new JsonParser().parse(new String(encoded,StandardCharsets.UTF_8));
                    if(!value.isJsonObject())throw new IOException("invalid shader clock sample");
                    NativeShaderClock clock=NativeShaderClock.archived(value.getAsJsonObject(),session);
                    Point point=new Point(time,clock);
                    if(!Double.isFinite(time)||!bits(time,mapping.time(clock))||time<chunk.start||time>=chunk.end)
                        throw new IOException("shader clock point differs from presentation mapping or index");
                    ordered(previous,point);result.add(point);previous=point;
                }
                if(input.read()!=-1||!bits(result.get(0).time,chunk.start))throw new IOException("shader clock block differs from its index");
            }catch(IllegalArgumentException invalid){throw new IOException("invalid archived shader clock",invalid);}
            return result;
        }
        private List<Point> block(int index) throws IOException {
            List<Point> points=cache.get(index);if(points!=null)return points;
            try(AppearanceAssets.Archive archive=new AppearanceAssets.Archive(file)) {
                if(!manifest.equals(archive.manifestHash))throw new IOException("shader clock archive changed after mount");
                points=decode(archive,chunks.get(index));
            }
            if(cache.size()==2)cache.remove(cache.keySet().iterator().next());cache.put(index,points);return points;
        }
        /** All observations at this exact presentation instant, in native
         * observation order. No interpolation/hold across missing frames. */
        List<NativeShaderClock> observationsAt(double time) throws IOException {
            if(!Double.isFinite(time)||time<0||time>86_400_000)return Collections.emptyList();
            int low=0,high=chunks.size()-1,index=-1;
            while(low<=high){int middle=(low+high)>>>1;if(chunks.get(middle).start<=time){index=middle;low=middle+1;}else high=middle-1;}
            if(index<0||time>=chunks.get(index).end)return Collections.emptyList();
            List<NativeShaderClock> result=new ArrayList<NativeShaderClock>();
            for(Point point:block(index))if(bits(point.time,time))result.add(point.clock);
            return Collections.unmodifiableList(result);
        }
        double firstTime(){return firstTime;}
        double lastTime(){return lastTime;}
        private static double number(JsonObject value,String key) throws IOException {
            JsonElement input=value.get(key);
            if(input==null||!input.isJsonPrimitive()||!input.getAsJsonPrimitive().isNumber()||!Double.isFinite(input.getAsDouble()))
                throw new IOException("invalid shader clock index number");
            return input.getAsDouble();
        }
    }
}
