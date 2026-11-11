package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.ArcticClient;
import java.io.File;
import java.awt.image.BufferedImage;

import com.arcticlauncher.client.Lang;
/** JNI entry points; every core UI call is serialized on the native render thread. */
public final class NativeBridge {
    private static BedrockPlatform platform;
    private static long nextTick;
    private static final java.util.LinkedHashMap<String,Integer> engineSizes=new java.util.LinkedHashMap<String,Integer>();
    private static int engineBytes;
    private static java.util.Set<String> bodyTexturePins=java.util.Collections.emptySet();
    private static long frameProbeSequence;
    private static String frameCaptureMetadata;
    private static boolean captureMetadataWanted,cleanCaptureView;
    private static boolean nativeCosmeticsActive;
    private static boolean geometryContext;
    private static final WorldState WORLD=new WorldState();
    private static final EmotePrograms EMOTE_PROGRAMS=new EmotePrograms();
    private static final com.google.gson.Gson ATTACHMENT_JSON=new com.google.gson.Gson();

    public static void init(String configDir, String version) {
        init(configDir, version, "ArcticGuest"); // i18n-ignore: an account name
    }

    public static void init(String configDir, String version, String offlineName) {
        init(configDir,version,offlineName,"");
    }

    public static void init(String configDir, String version, String offlineName, String javaReferenceJar) {
        if (platform != null) throw new IllegalStateException("client already initialized");
        platform = new BedrockPlatform(new File(configDir), version, offlineName,
            javaReferenceJar==null||javaReferenceJar.isEmpty()?null:new File(javaReferenceJar));
        ArcticClient.init(platform);
    }

    public static boolean menuOpen() { return platform != null && platform.page != null; }
    /** Asset-only copy; neither a glint selector nor a render/clock receipt. */
    public static byte[] armorGlintRgba() {
        requirePresentationContext();
        return platform==null?null:platform.javaReferenceAssets.armorGlintRgba();
    }
    /** Identifies only a probe layer included in the most recently returned pixels. */
    public static long frameProbeSequence() { return frameProbeSequence; }
    public static void captureView(boolean metadata,boolean clean) {captureMetadataWanted=metadata;cleanCaptureView=clean;}
    public static String frameCaptureMetadata() {return frameCaptureMetadata;}
    /** Explicit client-thread geometry request. A world gap or a stale native
     * consumer receives no packet; neither pixels nor body plans are changed. */
    public static byte[] worldGeometry(String session,long sequence) {
        requirePresentationContext();
        WorldFrame world=platform==null?null:platform.world;
        if(!WORLD.renderable||world==null||!platform.worldInputAvailable
                ||!world.session.equals(session)||world.sequence!=sequence)return null;
        return geometryPacket(world);
    }
    /** Serialized client-thread staging of one exact native observation. No
     * depth readback, software world publication, body plan or capture receipt
     * is created. Never call this mutable client path from a native draw worker. */
    public static byte[] worldGeometryFromFrame(String json,String session,long sequence) {
        requirePresentationContext();
        if(platform==null)return null;
        try {
            WorldFrame world=WorldFrame.parseGeometry(json);
            if(session==null||!world.session.equals(session)||world.sequence!=sequence)
                throw new IllegalArgumentException("geometry input belongs to another native world packet");
            return geometryPacket(world);
        } catch(RuntimeException|Error error) {
            platform.avatarSimplePhases.reset();throw error;
        }
    }
    private static byte[] geometryPacket(WorldFrame world) {
        requirePresentationContext();
        WorldFrame previous=platform.world;boolean available=platform.worldInputAvailable;
        geometryContext=true;
        try {
            // Appearance/replay helpers read this scoped world, including its
            // exact rational time. WORLD and presentation readiness stay intact.
            platform.world=world;platform.worldInputAvailable=true;
            return world.geometry(platform);
        } catch(RuntimeException|Error error) {
            platform.avatarSimplePhases.reset();throw error;
        } finally {
            platform.world=previous;platform.worldInputAvailable=available;geometryContext=false;
        }
    }
    private static void requirePresentationContext() {
        if(geometryContext)throw new IllegalStateException("presentation mutation during geometry staging");
    }
    /** User preference; the native side applies its separate probe policy. */
    public static boolean nativeCosmeticsRequested() {
        return platform!=null && ArcticClient.config()!=null && ArcticClient.config().bedrockNativeCosmetics;
    }
    /** Actual mode feedback, serialized on the same client thread as bodyPlan. */
    public static void nativeCosmeticsMode(boolean enabled) {nativeCosmeticsActive=enabled;}
    /** An engine adapter must supply its actual highest-priority-first roots.
     * Optional canonical item -> atlas-key bindings must agree with resolved
     * native observations when present. A raw Item::mIconName is insufficient.
     * No directory order or icon key is inferred. */
    public static void bindItemResources(String session,String json) throws java.io.IOException {
        checkAppearanceSession(session);
        if(!platform.worldInputAvailable)throw new java.io.IOException("item resources need a current world snapshot");
        try {platform.itemSprites.bind(session,json);}
        catch(RuntimeException|StackOverflowError invalid) {throw new java.io.IOException("invalid item-resource binding",invalid);} // i18n-ignore
    }
    public static String itemResourceState() {
        return platform==null?"{\"stage\":\"unbound\"}":platform.itemSprites.status().toString();
    }
    /** Pure math callback: safe for an attached native draw thread. It accesses
     * only immutable leased programs, copied timing and caller-owned matrices. */
    public static double[] sampleEmotePose(String token,String session,String actor,String timing,float[] baseline) {
        return EMOTE_PROGRAMS.sample(token,session,actor,timing,baseline);
    }
    static boolean emotePoseMatches(WorldFrame.EmotePose pose,WorldFrame world,String actor,com.arcticlauncher.client.looks.Animation animation) {
        return EMOTE_PROGRAMS.matches(pose,world.session,actor,world.cosmeticTimeExactMillis(),animation)
            &&acknowledgedEmote(pose,world,actor)==animation;
    }
    static com.arcticlauncher.client.looks.Animation acknowledgedEmote(WorldFrame.EmotePose pose,WorldFrame world,String actor) {
        return EMOTE_PROGRAMS.acknowledged(pose,world.session,actor,world.cosmeticTimeExactMillis());
    }
    /** Portable appearance capture on this bridge's client thread. The caller
     * supplies a new file; no auth envelope or account proof is archived. */
    public static void saveAppearance(String output) throws java.io.IOException {
        if(platform==null||!WORLD.renderable||platform.world==null)throw new java.io.IOException("appearance capture needs a current world layer");
        if(platform.replayAppearanceActive()) {platform.saveReplayAppearance(new File(output));return;}
        platform.saveNativeAppearance(new File(output));
    }
    /** Serialized client-thread APIs. A mount does not bypass native draw gates. */
    public static void checkAppearanceSession(String session) throws java.io.IOException {
        if(platform==null||platform.world==null||!platform.world.session.equals(session))throw new java.io.IOException("appearance command belongs to a different Java world snapshot");
    }
    public static void loadAppearance(String input,String session) throws java.io.IOException {
        loadAppearance(input,session,null);
    }
    public static void loadAppearance(String input,String session,String owner) throws java.io.IOException {
        if(platform==null)throw new java.io.IOException("client is not initialized");
        platform.mountReplayAppearance(new File(input),session,owner);EMOTE_PROGRAMS.clear();bodyTexturePins=java.util.Collections.emptySet();
    }
    public static void appearanceTime(String session,long timeMillis) throws java.io.IOException {
        appearanceTime(session,timeMillis,null);
    }
    public static void appearanceTime(String session,long timeMillis,String owner) throws java.io.IOException {
        if(platform==null)throw new java.io.IOException("client is not initialized");platform.replayAppearanceTime(session,timeMillis,owner);
    }
    public static void clearAppearance(String session) throws java.io.IOException {
        clearAppearance(session,null);
    }
    public static void clearAppearance(String session,String owner) throws java.io.IOException {
        if(platform==null||platform.world==null||!platform.world.session.equals(session))throw new java.io.IOException("appearance clear belongs to a different world");
        platform.clearReplayAppearance(owner);EMOTE_PROGRAMS.clear();bodyTexturePins=java.util.Collections.emptySet();
    }
    public static String appearanceState() {
        com.google.gson.JsonObject state=new com.google.gson.JsonObject();
        state.addProperty("session",platform==null||platform.world==null?null:platform.world.session);
        state.addProperty("loaded",platform!=null&&platform.replayAppearanceActive());
        state.addProperty("localPlayer",platform==null||platform.world==null?null:platform.world.localPlayer);
        state.addProperty("worldCurrent",platform!=null&&platform.worldInputAvailable);
        if(platform!=null&&platform.world!=null&&platform.world.nativeTiming!=null)state.add("worldTick",platform.world.nativeTiming.get("ticks"));
        if(platform!=null&&platform.replayAppearanceActive())state.add("appearance",platform.replayAppearanceStatus());
        // Explicit null distinguishes a confirmed absent writer from a reply
        // that omitted the history contract entirely during owner recovery.
        state.add("history",platform==null?null:platform.appearanceHistoryStatus());
        if(platform!=null)state.add("itemResources",platform.itemSprites.status());
        if(platform!=null)state.add("javaReferenceAssets",platform.javaReferenceAssets.status());
        if(platform!=null)state.add("javaEnchantmentComponents",JavaEnchantmentComponents.status(platform.world));
        if(platform!=null)state.add("javaItemFoil",JavaItemFoil.status(platform.world));
        if(platform!=null)state.add("javaCapeLayer",JavaCapeLayer.status(platform.world));
        state.add("emotePrograms",EMOTE_PROGRAMS.status());
        state.addProperty("nativeCosmeticsRequested",nativeCosmeticsRequested());
        state.addProperty("nativeCosmeticsActive",nativeCosmeticsActive);
        state.addProperty("nativeCosmeticsVerified",false);
        return state.toString();
    }
    public static void beginAppearanceHistory(String output,String session,long origin,String owner) throws java.io.IOException {
        if(platform==null)throw new java.io.IOException("client is not initialized");
        platform.beginAppearanceHistory(new File(output),session,origin,owner);
    }
    public static void beginAnchoredAppearanceHistory(String output,String session,long relayTime,String owner,String json) throws java.io.IOException {
        if(platform==null||json==null||json.length()>4096)throw new java.io.IOException("invalid appearance recording anchor");
        platform.beginAnchoredAppearanceHistory(new File(output),session,relayTime,owner,new com.google.gson.JsonParser().parse(json).getAsJsonObject());
    }
    public static void finishAppearanceHistory(String session,String owner,boolean cancel) throws java.io.IOException {
        if(platform==null)throw new java.io.IOException("client is not initialized");platform.finishAppearanceHistory(session,owner,cancel);
    }
    /** Prepare copied replay attachment values on the client thread. Native
     * consumers only use this leased plan; they never open its archive. */
    public static String attachmentPlan() {
        requirePresentationContext();
        com.google.gson.JsonObject plan=new com.google.gson.JsonObject();plan.addProperty("schema",1);
        WorldFrame world=platform==null?null:platform.world;
        boolean active=world!=null&&platform.replayAppearanceActive();plan.addProperty("active",active);
        if(!active)return plan.toString();
        plan.addProperty("session",world.session);plan.addProperty("manifestHash",platform.replayAppearanceManifestHash());
        CosmeticClock.Sample time=world.replayCosmeticSample();
        boolean ready=nativeCosmeticsActive&&WorldProbe.bodyControl().allowReadiness()&&platform.worldInputAvailable&&time!=null;
        plan.addProperty("ready",ready);com.google.gson.JsonArray actors=new com.google.gson.JsonArray();plan.add("actors",actors);
        if(ready) {
            plan.addProperty("timeMs",time.timeMs);plan.addProperty("timeRemainder",time.remainder);plan.addProperty("timeDenominator",time.denominator);
            plan.addProperty("nativeTick",world.nativeTiming.get("ticks").getAsLong());plan.addProperty("partial",world.nativeTiming.get("partial").getAsFloat());
            com.google.gson.JsonArray origin=new com.google.gson.JsonArray();for(int i=0;i<3;i++)origin.add(new com.google.gson.JsonPrimitive((float)world.camera[i]));plan.add("cameraOrigin",origin);
            com.google.gson.JsonObject packet=world.nativeTiming.getAsJsonObject("captureControls").getAsJsonObject("packetDispatch");
            com.google.gson.JsonObject marker=new com.google.gson.JsonObject();
            for(String key:new String[]{"sequence","requestId","marker","generation","replaySequence","observationSequence"})marker.add(key,packet.get(key));
            plan.add("marker",marker);
            for(WorldFrame.Player player:world.players)if(player.visible&&Integer.valueOf(63).equals(player.availableParts)) {
                AttachmentSample sample=platform.recordedAttachmentPose(java.util.UUID.fromString(player.id));if(sample==null)continue;
                sample=sample.forNativeCape(platform.nativeCapeReplayIdentity(player)).forNativeWings(platform.nativeWingReplayRequested(player));
                float[] root=platform.recordedModelRoot(java.util.UUID.fromString(player.id));if(platform.recordedModelRoots()&&root==null)continue;
                com.google.gson.JsonObject actor=new com.google.gson.JsonObject();actor.addProperty("id",player.id);
                if(root!=null)actor.add("root",ATTACHMENT_JSON.toJsonTree(root));
                com.google.gson.JsonArray capeColors=new com.google.gson.JsonArray(),wingColors=new com.google.gson.JsonArray();
                java.util.Set<String> colorGeometry=new java.util.HashSet<String>();
                for(AttachmentSample.Palette palette:sample.palettes)if((palette.slot==AttachmentSample.CAPE_SLOT||palette.slot==AttachmentSample.WING_SLOT)
                        &&colorGeometry.add(palette.slot+":"+palette.geometry)) {
                    boolean wing=palette.slot==AttachmentSample.WING_SLOT;
                    float[] rgba=wing?platform.nativeWingReplayColor(player,palette.geometry):platform.nativeCapeReplayColor(player,palette.geometry);
                    if(rgba==null)continue;
                    com.google.gson.JsonObject color=new com.google.gson.JsonObject();color.addProperty("geometry",palette.geometry);
                    color.addProperty("slot",palette.slot);color.addProperty("nativeCape",palette.nativeCape);
                    color.add("rgba",ATTACHMENT_JSON.toJsonTree(rgba));(wing?wingColors:capeColors).add(color);
                    Boolean dithering=wing?platform.nativeWingReplayDithering(player,palette.geometry):platform.nativeCapeReplayDithering(player,palette.geometry);
                    if(dithering!=null)color.addProperty("ditheringEnabled",dithering);
                    ActorShaderInputs shaderInputs=wing?platform.nativeWingReplayShaderInputs(player,palette.geometry):platform.nativeCapeReplayShaderInputs(player,palette.geometry);
                    if(shaderInputs!=null)color.add("shaderInputs",ATTACHMENT_JSON.toJsonTree(shaderInputs));
                }
                actor.add("capeColors",capeColors);
                actor.add("wingColors",wingColors);
                actor.add("samples",ATTACHMENT_JSON.toJsonTree(sample));actors.add(actor);
            }
        }
        String result=plan.toString();
        if(result.getBytes(java.nio.charset.StandardCharsets.UTF_8).length>8*1024*1024)
            throw new IllegalStateException("attachment replay plan exceeds 8 MiB");
        return result;
    }
    /** Owned texture readiness, queried by the experimental native body binding. */
    public static String bodyPlan() {
        requirePresentationContext();
        com.google.gson.JsonObject plan=new com.google.gson.JsonObject();
        plan.addProperty("schema",1);
        WorldFrame world=platform==null?null:platform.world;
        plan.addProperty("session",world==null?"":world.session);
        com.google.gson.JsonArray players=new com.google.gson.JsonArray();plan.add("players",players);
        com.google.gson.JsonArray capes=new com.google.gson.JsonArray();plan.add("capes",capes);
        com.google.gson.JsonArray hiddenCapes=new com.google.gson.JsonArray();plan.add("hiddenCapes",hiddenCapes);
        com.google.gson.JsonArray elytras=new com.google.gson.JsonArray();plan.add("elytras",elytras);
        com.arcticlauncher.client.looks.Looks looks=ArcticClient.looks();
        ProbeBodyControl.State control=WorldProbe.bodyControl();
        boolean allow=nativeCosmeticsActive && control.allowReadiness();
        java.util.List<EmotePrograms.Seed> seeds=new java.util.ArrayList<EmotePrograms.Seed>();
        if(allow&&world!=null&&platform.worldInputAvailable)for(WorldFrame.Player player:world.players) {
            if(!player.visible||!Integer.valueOf(63).equals(player.availableParts)||player.partMatrices==null)continue;
            EmotePrograms.Seed seed=platform.emoteSeed(java.util.UUID.fromString(player.id));if(seed!=null)seeds.add(seed);
        }
        java.util.Map<String,String> emoteTokens;
        if(world!=null&&platform.worldInputAvailable&&allow)emoteTokens=EMOTE_PROGRAMS.publish(world.session,seeds);
        else {EMOTE_PROGRAMS.clear();emoteTokens=java.util.Collections.emptyMap();}
        java.util.Set<String> pins=new java.util.HashSet<String>();
        // A missing matched depth frame hides geometry, but does not unload a
        // texture. WorldState retains only this session during a live-world
        // gap; disconnect/session changes still clear this readiness normally.
        if(world!=null)for(WorldFrame.Player p:world.players) {
            if(!p.visible || p.availableParts==null || p.availableParts!=63 || p.skin==null
                    || !"native-model-draw".equals(p.modelSource))continue;
            // Keep both the currently acknowledged draw and the next plan's
            // texture alive while asynchronous native/depth frames hand over.
            if(p.replaceBody && p.skin.matches("engine:skin/[0-9a-f]{64}")
                    && platform.textures.containsKey(p.skin))pins.add(p.skin);
            // A mode change can precede presentation of an already suppressed
            // native draw. Keep its acknowledged texture until that frame retires.
            if(!allow || looks==null&&!platform.replayAppearanceActive())continue;
            BedrockPlatform.BodyTexture texture=platform.bodyTexture(p,looks);
            if(texture!=null) {
                com.google.gson.JsonObject entry=new com.google.gson.JsonObject();
                entry.addProperty("id",p.id);entry.addProperty("skin",texture.key);
                entry.addProperty("slim",texture.slim);players.add(entry);
                entry.addProperty("suppressionAlpha",control.alpha);
                entry.addProperty("suppressionMethod",control.method);
                String token=emoteTokens.get(p.id);if(token!=null)entry.addProperty("emoteToken",token);
                if(texture.key.startsWith("engine:"))pins.add(texture.key);
            }
            BedrockPlatform.CapeTexture cape=platform.capeTexture(p,world.cosmeticTimeMillis(),looks);
            if(cape!=null) {
                com.google.gson.JsonObject entry=new com.google.gson.JsonObject();
                entry.addProperty("id",p.id);entry.addProperty("cape",cape.hash);
                entry.addProperty("texture",cape.key);entry.addProperty("nativeCape",cape.nativeCape);capes.add(entry);
            }
            String hiddenCape=platform.hiddenCape(p);
            if(hiddenCape!=null) {
                com.google.gson.JsonObject entry=new com.google.gson.JsonObject();
                entry.addProperty("id",p.id);entry.addProperty("nativeCape",hiddenCape);hiddenCapes.add(entry);
            }
            BedrockPlatform.ElytraTexture wing=platform.elytraTexture(p,world.cosmeticTimeMillis(),looks);
            if(wing!=null) {
                com.google.gson.JsonObject entry=new com.google.gson.JsonObject();
                entry.addProperty("id",p.id);entry.addProperty("cape",wing.hash);
                entry.addProperty("texture",wing.key);elytras.add(entry);
            }
        }
        bodyTexturePins=pins;
        return plan.toString();
    }

    /** Engine ABI input, dispatched only on this bridge's render thread. */
    public static void worldFrame(String json,float[] terrainDepth) {
        requirePresentationContext();
        WorldFrame next=json==null?null:WorldFrame.parse(json,terrainDepth);
        updateWorld(WORLD.frame(next));
    }
    public static void worldGap(boolean nativeWorldPresent) {requirePresentationContext();updateWorld(WORLD.gap(nativeWorldPresent));}
    private static void updateWorld(boolean changed) {
        platform.world=WORLD.current;
        platform.worldInputAvailable=WORLD.renderable;
        if(changed||!platform.worldInputAvailable) {
            platform.avatarSimplePhases.reset();platform.softwareAvatarSimplePhases.reset();
        }
        if(changed||!platform.worldInputAvailable)EMOTE_PROGRAMS.clear();
        if(changed) {
            EMOTE_PROGRAMS.forget();
            platform.itemSprites.clear();
            platform.appearanceHistoryWorldEnded();
            platform.clearReplayAppearance();
            bodyTexturePins=java.util.Collections.emptySet();
            if(ArcticClient.looks()!=null)ArcticClient.looks().cosmetics().clearPlayback();
        }
        platform.itemSprites.observe(platform.world,platform.worldInputAvailable);
    }
    public static void engineTexture(String key,int width,int height,byte[] rgba) {
        // Engine keys identify immutable owned image content. Refresh recency
        // without repeatedly decoding the same skin on every native poll.
        if(key!=null && key.matches("engine:(?:skin|cape)/[0-9a-f]{64}")
                && width>0 && width<=1024 && height>0 && height<=1024
                && rgba!=null && rgba.length==width*height*4
                && engineSizes.containsKey(key) && platform.textures.containsKey(key)) {
            Integer bytes=engineSizes.remove(key);engineSizes.put(key,bytes);return;
        }
        BufferedImage image=engineImage(key,width,height,rgba);
        int bytes=image.getWidth()*image.getHeight()*4;
        Integer old=engineSizes.remove(key);if(old!=null)engineBytes-=old;
        while(!engineSizes.isEmpty() && (engineBytes+bytes>64*1024*1024 || engineSizes.size()>=128)) {
            java.util.Iterator<java.util.Map.Entry<String,Integer>> entries=engineSizes.entrySet().iterator();
            boolean removed=false;
            while(entries.hasNext()) {
                java.util.Map.Entry<String,Integer> first=entries.next();
                // Never evict a skin still authorizing an owned native body
                // replacement. Decline this upload if all resident keys are pinned.
                if(bodyTexturePins.contains(first.getKey()))continue;
                engineBytes-=first.getValue();platform.textures.remove(first.getKey());entries.remove();removed=true;break;
            }
            if(!removed) {
                if(old!=null) {engineSizes.put(key,old);engineBytes+=old;}
                return;
            }
        }
        platform.textures.put(key,image);engineSizes.put(key,bytes);engineBytes+=bytes;
    }
    /** Decode an owned native image without initializing accounts or the client. */
    static BufferedImage engineImage(String key,int width,int height,byte[] rgba) {
        if(key==null || !key.matches("engine:[A-Za-z0-9_/-]{1,120}") || width<1 || height<1 || width>1024 || height>1024
                || rgba==null || rgba.length!=width*height*4) throw new IllegalArgumentException("invalid engine texture");
        BufferedImage image=new BufferedImage(width,height,BufferedImage.TYPE_INT_ARGB);
        int[] pixels=new int[width*height];
        for(int i=0;i<pixels.length;i++) pixels[i]=((rgba[i*4+3]&255)<<24)|((rgba[i*4]&255)<<16)|((rgba[i*4+1]&255)<<8)|(rgba[i*4+2]&255);
        image.setRGB(0,0,width,height,pixels,0,width);
        if(key.startsWith("engine:skin/")) {
            image=com.arcticlauncher.client.looks.SkinFormat.modern(image);
            if(image==null)throw new IllegalArgumentException("invalid engine skin dimensions");
        }
        return image;
    }

    /** Width/height and mouse are world-surface pixels. The shared UI keeps a
     * separate GUI scale. Events are {kind, value} native-window pairs. */
    public static byte[] frame(int width, int height, int fps, int mx, int my, boolean[] keys, int[] events) {
        requirePresentationContext();
        frameProbeSequence=0;
        frameCaptureMetadata=null;
        if (platform == null) throw new IllegalStateException("client not initialized");
        if(width<1||width>3840||height<1||height>2160)throw new IllegalArgumentException("invalid presentation surface");
        if (keys.length != 256 || events.length % 2 != 0) throw new IllegalArgumentException("invalid input packet");
        final int guiScale=2,uiWidth=(width+guiScale-1)/guiScale,uiHeight=(height+guiScale-1)/guiScale;
        mx=Math.floorDiv(mx,guiScale);my=Math.floorDiv(my,guiScale);
        boolean resized = uiWidth != platform.width || uiHeight != platform.height;
        platform.width = uiWidth; platform.height = uiHeight; platform.fps = fps; platform.keys = keys;
        // A resized surface must not authorize fresh native suppression using
        // an old-size world/depth frame. Keep the session while awaiting a new
        // native observation, just as for another temporary world-frame gap.
        if(WORLD.renderable&&platform.world!=null&&(platform.world.width!=width||platform.world.height!=height))worldGap(true);
        Runnable task;
        while ((task = platform.tasks.poll()) != null) task.run();
        if (resized && platform.page != null) platform.page.init(uiWidth, uiHeight);
        for (int i = 0; i < events.length; i += 2) input(events[i], events[i + 1], mx, my);
        WorldProbe.pollEmote(platform);
        ArcticClient.frame(menuOpen());
        long now = System.nanoTime();
        // Do not replay a backlog of ticks after minimizing or pausing at a debugger.
        if (now >= nextTick) {
            ArcticClient.tick(menuOpen());
            if (platform.page != null) platform.page.tick();
            nextTick = now + 50_000_000L;
        }
        platform.sampleAppearanceHistory();
        try (SoftwareGfx gfx = new SoftwareGfx(width, height, platform, guiScale)) {
            WorldFrame world=WORLD.renderable?platform.world:null;
            if(world!=null && world.width==width && world.height==height) {
                try {
                    boolean renderLayer=WorldProbe.bodyControl().renderLayer();
                    if(!renderLayer)platform.softwareAvatarSimplePhases.reset();
                    BufferedImage layer=renderLayer?world.render(platform)
                        :new BufferedImage(width,height,BufferedImage.TYPE_INT_ARGB);
                    long probe=WorldProbe.capture(world,layer,platform);
                    gfx.worldImage(layer);
                    frameProbeSequence=probe;
                    if(captureMetadataWanted) {
                        com.google.gson.JsonObject appearance=platform.replayAppearanceStatus();
                        if(appearance!=null)appearance.addProperty("renderReady",renderLayer&&world.recordedAppearanceReady(platform));
                        frameCaptureMetadata=CaptureFrame.metadata(world,cleanCaptureView,renderLayer,appearance);
                    }
                }
                catch(IllegalArgumentException e) {
                    platform.avatarSimplePhases.reset();
                    platform.softwareAvatarSimplePhases.reset();
                    platform.world=null;platform.log(true,"World rendering rejected: "+e.getMessage());
                }
            } else {platform.softwareAvatarSimplePhases.reset();}
            if(cleanCaptureView) { /* Export world geometry without Arctic HUD/editor pixels. */ }
            else if (platform.page != null) platform.page.render(gfx, mx, my);
            else {
                ArcticClient.hud().render(gfx, ArcticClient.style(), false);
                gfx.text(Lang.t("platform.arctic_bedrock_native_preview_right_shift"), 6, uiHeight - 16, 0xFFC4D8EB, true);
            }
            return rgba(gfx.image());
        }
    }

    private static void input(int kind, int value, int mx, int my) {
        if (kind == 0) {
            int key = coreKey(value);
            if (platform.page != null) platform.page.keyPressed(key, value);
            else ArcticClient.screenKeyPressed(key);
        } else if (kind == 1 && platform.page != null) {
            platform.page.charTyped(value);
        } else if (kind == 2) {
            if (platform.page != null) platform.page.mouseClicked(mx, my, value);
            else ArcticClient.mousePressed(value);
        } else if (kind == 3 && platform.page != null) {
            platform.page.mouseReleased(mx, my, value);
        } else if (kind == 4 && platform.page != null) {
            platform.page.mouseScrolled(mx, my, value / 120.0);
        } else if (kind == 5 && platform.page != null) {
            platform.page.mouseDragged(mx, my, value);
        }
    }

    public static int coreKey(int vk) {
        return BedrockKeys.core(vk);
    }

    static byte[] rgba(BufferedImage image) {
        int width=image.getWidth(),height=image.getHeight();
        byte[] rgba = new byte[width*height*4];
        // A single row also handles arbitrary raster offsets/layouts. Avoid a
        // second complete ARGB surface (another 32 MiB at 3840x2160) per frame.
        int[] row=new int[width];
        for(int y=0;y<height;y++) {
            image.getRGB(0,y,width,1,row,0,width);
            for(int x=0;x<width;x++) {
                int argb=row[x],offset=(y*width+x)*4;
                rgba[offset] = (byte) (argb >> 16);
                rgba[offset + 1] = (byte) (argb >> 8);
                rgba[offset + 2] = (byte) argb;
                rgba[offset + 3] = (byte) (argb >>> 24);
            }
        }
        return rgba;
    }
}
