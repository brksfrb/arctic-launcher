package com.arcticlauncher.client.looks;

import com.google.gson.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicLong;

import com.arcticlauncher.client.Lang;
/** Observed logical changes on the client thread, raw copies on one bounded
 * writer. A missing asset or overflow fails the take; it never skips an event
 * and seals a supposedly complete timeline. This does not record world packets. */
public final class AppearanceHistory {
    private final AppearanceAssets source;
    private final JsonObject association;
    private final File output;
    private final Path temporary;
    private final AppearanceAssets spool;
    private final ThreadPoolExecutor writer;
    private final List<JsonObject> events=new ArrayList<JsonObject>();
    private final SortedSet<AppearanceAssets.Ref> files=new TreeSet<AppearanceAssets.Ref>();
    private final SortedMap<String,JsonArray> tracks=new TreeMap<String,JsonArray>();
    private volatile String stage="recording",error;
    private final AtomicLong pendingBytes=new AtomicLong();
    private volatile long assetBytes,endTime;
    private volatile int count,trackChunks;
    private String changeKey,session,localPlayer;
    private long lastTime=-1,firstTime=-1;

    public AppearanceHistory(AppearanceAssets source,File directory,File output) throws IOException {
        this(source,directory,output,null);
    }
    public AppearanceHistory(AppearanceAssets source,File directory,File output,JsonObject association) throws IOException {
        if(source==null||output==null||!output.isAbsolute()||!output.getName().endsWith(".arctic-looks.zip")
                ||output.exists()||output.getParentFile()==null||!output.getParentFile().isDirectory())throw new IOException("appearance history needs a new absolute .arctic-looks.zip file");
        this.source=source;this.output=output;
        if(association!=null&&association.toString().getBytes(StandardCharsets.UTF_8).length>4096)throw new IOException("appearance recording association exceeds 4 KiB");
        this.association=association==null?null:new JsonParser().parse(association.toString()).getAsJsonObject();
        temporary=Files.createTempDirectory(directory.toPath().toAbsolutePath().normalize(),".appearance-history-");spool=new AppearanceAssets(temporary.toFile());
        writer=new ThreadPoolExecutor(1,1,0,TimeUnit.MILLISECONDS,new ArrayBlockingQueue<Runnable>(32),task->{
            Thread thread=new Thread(task,"arctic-appearance-history");thread.setDaemon(true);return thread;
        });
    }
    public boolean recording() {return "recording".equals(stage)&&error==null;}
    public boolean busy() {return error==null&&(recording()||"finishing".equals(stage));}
    public void accept(AppearanceSnapshot.Captured captured) throws IOException {
        if(!recording())throw new IOException(error==null?"appearance history is not recording":error);
        JsonObject state=captured.state;long time=state.get("timeMs").getAsLong();
        String world=state.get("session").getAsString(),local=state.get("localPlayer").isJsonNull()?null:state.get("localPlayer").getAsString();
        if(lastTime>=0&&(!world.equals(session)||!Objects.equals(local,localPlayer)||time<lastTime)) {
            cancel("appearance world, local identity or clock changed");throw new IOException(error); // i18n-ignore
        }
        if(firstTime<0)firstTime=time;
        lastTime=time;session=world;localPlayer=local;endTime=time;
        if(captured.changeKey.equals(changeKey))return;
        final byte[] json=state.toString().getBytes(StandardCharsets.UTF_8);final int bytes=json.length;
        if(count>=4096||bytes>1024*1024) {cancel("appearance history state budget exhausted");throw new IOException(error);}
        if(pendingBytes.addAndGet(bytes)>8*1024*1024) {
            pendingBytes.addAndGet(-bytes);cancel("appearance history pending state budget exhausted");throw new IOException(error);
        }
        count++;changeKey=captured.changeKey;
        final List<AppearanceAssets.Ref> references=new ArrayList<AppearanceAssets.Ref>(captured.files);
        enqueue(()->{
            for(AppearanceAssets.Ref ref:references)if(!files.contains(ref)) {
                byte[] raw=source.read(ref.kind,ref.hash);
                if(raw==null)throw new IOException("appearance history asset disappeared before copying: "+ref.kind+"/"+ref.hash);
                retain(ref,raw);
            }
            String hash=AppearanceAssets.digest("SHA-1",json);retain(new AppearanceAssets.Ref("asset",hash),json);
            JsonObject owned=new JsonObject();owned.addProperty("timeMs",time);owned.addProperty("stateHash",hash);
            // Several rendered changes at one held clock instant have one final
            // state. Assets copied from superseded states remain bounded.
            if(!events.isEmpty()&&events.get(events.size()-1).get("timeMs").getAsLong()==time)events.set(events.size()-1,owned);
            else events.add(owned);
        },bytes);
    }
    private void retain(AppearanceAssets.Ref ref,byte[] raw) throws IOException {
        if(files.contains(ref))return;
        if(files.size()>=4096||assetBytes+raw.length>256L*1024*1024||(files.size()+1)*192L+(events.size()+1)*96L+(trackChunks+1)*192L+512>1024*1024)
            throw new IOException("appearance history asset/index budget exhausted");
        spool.put(ref.kind,ref.hash,raw);files.add(ref);assetBytes+=raw.length;
    }
    /** Opaque adapter data: a bounded zlib block with a timed, hashed index.
     * Compression and file copies share the same serialized writer/queue. */
    public void appendTrack(String kind,byte[] raw,double start,double end,int frames) throws IOException {
        if(!recording()||kind==null||!kind.matches("[a-z0-9_]{1,40}")||raw==null||raw.length<1||raw.length>2*1024*1024
                ||!Double.isFinite(start)||!Double.isFinite(end)||start< -1000||end>86_401_000||end<=start||frames<1||frames>64)
            throw new IOException("invalid appearance adapter track block");
        int bytes=raw.length;
        if(pendingBytes.addAndGet(bytes)>8*1024*1024) {
            pendingBytes.addAndGet(-bytes);cancel("appearance history pending data budget exhausted");throw new IOException(error);
        }
        byte[] owned=raw.clone();
        enqueue(()->{
            ByteArrayOutputStream compressed=new ByteArrayOutputStream();
            try(java.util.zip.DeflaterOutputStream deflater=new java.util.zip.DeflaterOutputStream(compressed)) {deflater.write(owned);}
            byte[] payload=compressed.toByteArray();String hash=AppearanceAssets.digest("SHA-1",payload);
            retain(new AppearanceAssets.Ref("asset",hash),payload);
            JsonArray chunks=tracks.get(kind);if(chunks==null) {chunks=new JsonArray();tracks.put(kind,chunks);}
            if(trackChunks>=4096||chunks.size()>0&&chunks.get(chunks.size()-1).getAsJsonObject().get("endMs").getAsDouble()>start+0.000001)
                throw new IOException("appearance adapter track overlaps or exceeds its index budget");
            JsonObject chunk=new JsonObject();chunk.addProperty("hash",hash);chunk.addProperty("startMs",start);chunk.addProperty("endMs",end);chunk.addProperty("frames",frames);
            chunks.add(chunk);trackChunks++;
        },bytes);
    }
    public void finish(long time) throws IOException {
        if(!recording()||lastTime<0||time<lastTime||time>86_400_000)throw new IOException("appearance history cannot finish at that time");
        endTime=time;stage="finishing";
        enqueue(()->{
            JsonObject state=new JsonObject();state.addProperty("format",2);state.addProperty("rig",2);
            state.addProperty("session",session);state.addProperty("localPlayer",localPlayer);
            state.addProperty("timeMs",firstTime);state.addProperty("durationMs",time);
            JsonArray changes=new JsonArray();for(JsonObject event:events)changes.add(event);state.add("events",changes);
            JsonObject adapterTracks=new JsonObject();for(Map.Entry<String,JsonArray> entry:tracks.entrySet())adapterTracks.add(entry.getKey(),entry.getValue());
            state.add("tracks",adapterTracks);
            if(association!=null)state.add("source",association);
            spool.archive(output,state,files,(kind,hash)->{
                if(error!=null)throw new IOException(error);return spool.read(kind,hash);
            });if(error==null)stage="complete";
        });
        writer.shutdown();
    }
    private interface Write {void run() throws IOException;}
    private void enqueue(Write job) throws IOException {enqueue(job,0);}
    private void enqueue(Write job,int bytes) throws IOException {
        try {writer.execute(()->{
            try {if(error==null)job.run();}
            catch(Exception failure) {fail(failure.getMessage()==null?failure.toString():failure.getMessage());}
            finally {pendingBytes.addAndGet(-bytes);if(error!=null||"complete".equals(stage))cleanup();}
        });} catch(RejectedExecutionException full) {
            pendingBytes.addAndGet(-bytes);
            cancel("appearance writer queue is full or closed");throw new IOException(error,full);
        }
    }
    public void cancel(String reason) {
        if(!busy())return;
        if(error==null)error=reason;stage="failed";
        // If the queue is full/finishing, an already accepted task performs
        // cleanup in its finally block. An idle writer needs its own task.
        try {writer.execute(this::cleanup);}catch(RejectedExecutionException ignored) {}
        writer.shutdown();
    }
    private void fail(String reason) {if(error==null)error=reason;stage="failed";writer.shutdown();}
    private void cleanup() {
        try(DirectoryStream<Path> entries=Files.newDirectoryStream(temporary)) {
            for(Path file:entries)if(file.getFileName().toString().matches("(?:asset|texture)-[0-9a-f]{40}")
                    &&Files.isRegularFile(file,LinkOption.NOFOLLOW_LINKS))Files.deleteIfExists(file);
        } catch(IOException ignored) {return;}
        try {Files.deleteIfExists(temporary);}catch(IOException ignored) {}
    }
    public JsonObject status() {
        JsonObject state=new JsonObject();state.addProperty("stage",error==null?stage:"failed");state.addProperty("error",error);
        state.addProperty("path",output.getAbsolutePath());state.addProperty("eventsObserved",count);
        if(association!=null)state.add("source",association);
        state.addProperty("endTimeMs",endTime);state.addProperty("assetBytes",assetBytes);state.addProperty("pendingBytes",pendingBytes.get());state.addProperty("trackChunks",trackChunks);return state;
    }
}
