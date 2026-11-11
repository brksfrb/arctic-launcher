package com.arcticlauncher.bedrock;

import com.google.gson.*;
import java.awt.image.BufferedImage;
import java.io.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicLong;

/** Static item sprites from a bound engine pack stack and resolved native
 * key/frame selections (or optional explicit adapter keys).
 * Dynamic variants, tint, glint and 3D block items require engine bindings. */
final class ItemSprites {
    private static final long MAX_PIXELS=16L*1024*1024;
    private final BedrockPlatform platform;
    private final ThreadPoolExecutor loader=new ThreadPoolExecutor(1,1,0,TimeUnit.MILLISECONDS,
        new ArrayBlockingQueue<Runnable>(32),r->{Thread t=new Thread(r,"arctic-bedrock-item-assets");t.setDaemon(true);return t;});
    private volatile long generation;
    private Context active;
    private String stage="unbound",error;
    private String nativeRevision;
    private final AtomicLong pendingPixels=new AtomicLong();
    ItemSprites(BedrockPlatform platform){this.platform=platform;}
    private static final class Context {
        final String session;final PackAssets assets;final Map<String,String> registry;
        final Map<String,BufferedImage> images=new LinkedHashMap<String,BufferedImage>(16,.75f,true);
        final Set<String> pending=new HashSet<String>(),failed=new HashSet<String>(),keys=new HashSet<String>();
        long bytes;
        Context(String session,PackAssets assets,Map<String,String> registry){this.session=session;this.assets=assets;this.registry=registry;}
    }
    void clear(){generation++;loader.getQueue().clear();active=null;nativeRevision=null;stage="unbound";error=null;}
    void observe(WorldFrame frame,boolean fresh) {
        if(!fresh||frame==null||frame.itemResources==null) {if(nativeRevision!=null)clear();return;}
        WorldFrame.ResourceStack stack=frame.itemResources;String key=frame.session+":"+stack.revision;
        if(key.equals(nativeRevision))return;
        clear();
        if(stack.error!=null) {stage="unavailable";error=stack.error;nativeRevision=key;return;}
        List<List<File>> packs=new ArrayList<List<File>>();
        for(String[] roots:stack.packs) {
            List<File> directories=new ArrayList<File>();for(String root:roots)directories.add(new File(root));packs.add(directories);
        }
        try {prepare(frame.session,packs,Collections.<String,String>emptyMap(),true);}
        catch(IOException|RuntimeException|StackOverflowError failure) {stage="failed";error=message(failure);}
        nativeRevision=key;
    }
    void bind(String session,String json) throws IOException {
        if(json==null||json.length()>256*1024)throw new IOException("item-resource binding exceeds 256 KiB");
        JsonObject request=new JsonParser().parse(json).getAsJsonObject();
        if(!request.has("roots")||request.entrySet().size()!=(request.has("icons")?2:1))throw new IOException("item resources require roots and optional explicit registry icon keys");
        List<File> roots=new ArrayList<File>();
        JsonArray rootValues=request.getAsJsonArray("roots");
        if(rootValues.size()<1||rootValues.size()>64)throw new IOException("item resources require 1–64 ordered roots");
        for(JsonElement value:rootValues) {
            if(!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isString()||value.getAsString().length()>32768)throw new IOException("invalid pack root");
            roots.add(new File(value.getAsString()));
        }
        if(roots.isEmpty()||roots.size()>64)throw new IOException("item resources require 1–64 ordered roots");
        Map<String,String> icons=new LinkedHashMap<String,String>();
        JsonObject supplied=request.has("icons")?request.getAsJsonObject("icons"):new JsonObject();
        for(Map.Entry<String,JsonElement> entry:supplied.entrySet()) {
            if(icons.size()>=4096||!entry.getKey().matches("[a-z0-9_.-]{1,64}:[a-z0-9_./-]{1,128}")
                    ||!entry.getValue().isJsonPrimitive()||!entry.getValue().getAsJsonPrimitive().isString()
                    ||!entry.getValue().getAsString().matches("[A-Za-z0-9_.:/-]{1,193}"))throw new IOException("invalid or excessive item icon registry");
            icons.put(entry.getKey(),entry.getValue().getAsString());
        }
        List<List<File>> packs=new ArrayList<List<File>>();for(File root:roots)packs.add(Collections.singletonList(root));
        prepare(session,packs,icons,false);
    }
    private void prepare(String session,List<List<File>> packs,Map<String,String> icons,boolean grouped) throws IOException {
        clear();final long ticket=generation;stage="preparing";
        try {loader.execute(()-> {
            try {
                if(ticket!=generation)return;
                PackAssets assets;
                if(grouped)assets=PackAssets.grouped(packs);
                else {List<File> roots=new ArrayList<File>();for(List<File> pack:packs)roots.addAll(pack);assets=new PackAssets(roots);}
                Context ready=new Context(session,assets,Collections.unmodifiableMap(new LinkedHashMap<String,String>(icons)));
                platform.tasks.add(()-> {if(ticket==generation&&current(session)) {active=ready;stage="ready";error=null;}});
            } catch(IOException|RuntimeException|StackOverflowError failed) {platform.tasks.add(()-> {if(ticket==generation) {stage="failed";error=message(failed);}});}
        });} catch(RejectedExecutionException full) {stage="failed";error="item asset queue is full";throw new IOException(error,full);}
    }
    private boolean current(String session){return platform.world!=null&&platform.worldInputAvailable&&session.equals(platform.world.session);}
    private static String resolve(PackAssets assets,String key,Integer frame) throws IOException {
        for(int pack=0;pack<assets.size();pack++) {
            byte[] bytes=assets.read(pack,"textures/item_texture.json",8*1024*1024);if(bytes==null)continue;
            // The first definition in engine priority order wins. Unsupported
            // overrides cannot silently reveal a lower-priority vanilla image.
            String path=ItemAtlas.resolve(bytes,key,frame);if(path!=null)return path;
        }
        throw new IOException("observed item icon key is absent from the bound pack stack");
    }
    BufferedImage image(WorldFrame.ItemSlot slot) {
        Context context=active;if(context==null||slot==null||slot.empty||slot.item==null||!current(context.session))return null;
        // mIconName alone is not a resolved icon: component, armor-trim and
        // per-item overrides may choose another key. Native observations need
        // the static getter receipt; explicit adapter keys retain legacy support.
        String observed=slot.iconFrame==null?null:slot.iconKey,supplied=context.registry.get(slot.item),icon=observed!=null?observed:supplied;
        Integer frame=observed==null?null:slot.iconFrame;
        if(icon==null) {error="item icon needs a resolved engine selection for "+slot.item;return null;}
        if(observed!=null&&supplied!=null&&!observed.equals(supplied)) {error="item resource binding disagrees with the resolved native icon key";return null;}
        String item=slot.item+'\0'+icon+'\0'+frame;
        BufferedImage image=context.images.get(item);if(image!=null)return image;
        if(!context.keys.contains(item)&&context.keys.size()>=4096) {error="item sprite observation budget is exhausted";return null;}
        context.keys.add(item);
        if(context.failed.contains(item)||!context.pending.add(item))return null;
        final long ticket=generation;
        try {loader.execute(()-> {
            try {
                if(ticket!=generation)return;
                String path=resolve(context.assets,icon,frame);
                byte[] bytes=context.assets.read(path,1024*1024);if(bytes==null)throw new IOException("item texture is absent from the bound stack");
                BufferedImage pixels=AvatarRenderer.png(bytes);long size=(long)pixels.getWidth()*pixels.getHeight()*4;
                if(ticket!=generation)return;
                if(size>MAX_PIXELS)throw new IOException("item sprite exceeds its decoded pixel budget");
                long pending;
                do {
                    pending=pendingPixels.get();
                    if(pending+size>MAX_PIXELS) {platform.tasks.add(()->context.pending.remove(item));return;}
                } while(!pendingPixels.compareAndSet(pending,pending+size));
                platform.tasks.add(()-> {
                    try {
                        context.pending.remove(item);if(active!=context||!current(context.session))return;
                        while(!context.images.isEmpty()&&(context.bytes+size>MAX_PIXELS||context.images.size()>=512)) {
                            Iterator<BufferedImage> entries=context.images.values().iterator();BufferedImage old=entries.next();entries.remove();
                            context.bytes-=(long)old.getWidth()*old.getHeight()*4;
                        }
                        context.images.put(item,pixels);context.bytes+=size;
                    } finally {pendingPixels.addAndGet(-size);}
                });
            } catch(IOException|RuntimeException|StackOverflowError failed) {platform.tasks.add(()-> {context.pending.remove(item);context.failed.add(item);if(active==context)error=message(failed);});}
        });} catch(RejectedExecutionException full) {context.pending.remove(item);}
        return null;
    }
    JsonObject status() {
        JsonObject value=new JsonObject();value.addProperty("stage",stage);value.addProperty("error",error);
        value.addProperty("session",active==null?null:active.session);
        value.addProperty("nativeRevision",nativeRevision);
        value.addProperty("packInstances",active==null?0:active.assets.size());
        value.addProperty("staticBaseLayerOnly",true);value.addProperty("nativeItemRenderingVerified",false);
        value.addProperty("explicitRegistryItems",active==null?0:active.registry.size());
        value.addProperty("observedKeys",active==null?0:active.keys.size());
        value.addProperty("failedKeys",active==null?0:active.failed.size());
        value.addProperty("pendingSprites",active==null?0:active.pending.size());
        value.addProperty("decodedCacheBytes",active==null?0:active.bytes);
        value.addProperty("pendingUploadBytes",pendingPixels.get());
        value.addProperty("loadedSprites",active==null?0:active.images.size());value.addProperty("activeEngineStackVerified",false);return value;
    }
    private static String message(Throwable error) {
        String value=error.getMessage();if(value==null)value=error.getClass().getSimpleName();
        return value.length()>512?value.substring(0,512):value;
    }
}
