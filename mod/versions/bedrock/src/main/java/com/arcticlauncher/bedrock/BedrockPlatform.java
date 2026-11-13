package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.*;
import com.arcticlauncher.client.ui.Page;
import java.awt.image.BufferedImage;
import java.io.*;
import java.util.*;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentLinkedQueue;
import javax.imageio.ImageIO;

import com.arcticlauncher.client.Lang;
/** Shared client platform backed by native input/render events. No fabricated world state. */
public final class BedrockPlatform implements Platform {
    private static final com.google.gson.Gson NATIVE_STATE=new com.google.gson.GsonBuilder().serializeNulls().create();
    final ItemSprites itemSprites=new ItemSprites(this);
    final JavaSimplePhases avatarSimplePhases=new JavaSimplePhases();
    final JavaSimplePhases softwareAvatarSimplePhases=new JavaSimplePhases();
    final JavaReferenceAssets javaReferenceAssets;
    final Map<String, BufferedImage> textures = new ConcurrentHashMap<String, BufferedImage>();
    private final Map<String,String> javaTextureIds=new ConcurrentHashMap<String,String>();
    final Queue<Runnable> tasks = new ConcurrentLinkedQueue<Runnable>();
    final Map<String, AvatarRenderer.Cosmetic> cosmetics = new ConcurrentHashMap<String, AvatarRenderer.Cosmetic>();
    volatile WorldFrame world;
    private ReplayAppearance replayAppearance;
    private String replayAppearanceSession;
    private String replayAppearanceOwner;
    // Native history can deliver a draw from before a mount/clear. Keep its
    // immutable pixels until this world ends, with a separate bounded budget.
    private final Map<String,BufferedImage> retiredReplayTextures=new HashMap<String,BufferedImage>();
    private final Map<String,String> retiredReplayTextureIds=new HashMap<String,String>();
    private long retiredReplayBytes;
    private com.arcticlauncher.client.looks.AppearanceHistory appearanceHistory;
    private String historySession,historyOwner;
    private long historyClock,historyOrigin;
    private CapeHistory.Writer capeHistory;
    private WingHistory.Writer wingHistory;
    private BodyHistory.Writer bodyHistory;
    private AttachmentHistory.Writer attachmentHistory;
    private LayerColorHistory.Writer layerColorHistory;
    private EntityMaterialHistory.Writer entityMaterialHistory;
    private GlintHistory.Writer glintHistory;
    private ShaderClockHistory.Writer shaderClockHistory;
    private NativeSkinCapture historySkins;
    private final NativeEquipmentCapture equipmentCapture=new NativeEquipmentCapture();
    boolean worldInputAvailable;
    final File config;
    private final com.arcticlauncher.client.looks.AppearanceAssets appearanceAssets;
    final String version;
    private final String offlineName;
    private final UUID offlineId;
    boolean[] keys = new boolean[256];
    int fps, width = 800, height = 450;
    Page page;

    public BedrockPlatform(File config, String version) {
        this(config, version, "ArcticGuest"); // i18n-ignore: an account name
    }
    public BedrockPlatform(File config, String version, String offlineName) {
        this(config,version,offlineName,null);
    }
    public BedrockPlatform(File config, String version, String offlineName, File javaReferenceJar) {
        if (offlineName == null || !offlineName.matches("[A-Za-z0-9_]{3,16}")) {
            throw new IllegalArgumentException("offline name needs 3–16 letters, digits or underscores");
        }
        this.config = config;
        this.javaReferenceAssets=new JavaReferenceAssets(javaReferenceJar);
        this.appearanceAssets = new com.arcticlauncher.client.looks.AppearanceAssets(new File(config,"appearance-assets"));
        this.version = version;
        this.offlineName = offlineName;
        this.offlineId = UUID.nameUUIDFromBytes(("OfflinePlayer:" + offlineName) // i18n-ignore: Java's offline UUID
                .getBytes(java.nio.charset.StandardCharsets.UTF_8));
        if (!config.isDirectory() && !config.mkdirs()) throw new IllegalArgumentException("cannot create " + config);
    }
    @Override public String minecraftVersion() { return version + " (Bedrock native preview)"; }
    @Override public File configDir() { return config; }
    @Override public com.arcticlauncher.client.looks.AppearanceAssets appearanceAssets() { return appearanceAssets; }
    @Override public void log(boolean warning, String message) {
        System.err.println("[Arctic Bedrock] " + message); // i18n-ignore: log
        WorldProbe.log(message);
    }
    @Override public int fps() { return fps; }
    @Override public long animationTimeMillis() {
        WorldFrame frame=world;
        return frame==null?System.currentTimeMillis():frame.clockMs;
    }
    @Override public int ping() { return -1; }
    // Only a verified engine binding may supply inWorld, entity IDs or positions.
    @Override public boolean inWorld() { return world != null && world.local() != null; }
    @Override public double[] position() { WorldFrame f=world; return f==null||f.local()==null?null:f.local().position.clone(); }
    @Override public UUID worldPlayerId() { WorldFrame f=world; return f==null||f.localPlayer==null?null:UUID.fromString(f.localPlayer); }
    @Override public boolean localMoving() {
        WorldFrame frame=world;
        WorldFrame.Player local=frame==null?null:frame.local();
        return worldInputAvailable && local!=null && local.movement!=null && local.movement.active();
    }
    @Override public double[] camera() { WorldFrame f=world; return f==null?null:f.camera.clone(); }
    @Override public String biome() { return null; }
    @Override public String server() { WorldFrame f=world; return f==null?null:f.server; }
    @Override public long dayTime() { WorldFrame f=world; return f==null?-1:f.dayTime; }
    @Override public String dimension() { WorldFrame f=world; return f==null?"overworld":f.dimension; }
    @Override public int hurtTime() { return 0; }
    @Override public boolean hudHidden() { return false; }
    @Override public boolean hasFeatures() { return false; }
    @Override public boolean supportsGameWidget(String id) {return "armor".equals(id);}
    @Override public boolean supportsNativeCosmeticsPreview() {return true;}
    @Override public boolean smoothFont() { return false; }
    @Override public void setSmoothFont(boolean on) { unsupported("Changing the game's font"); }
    @Override public void setThirdPerson(boolean on) { unsupported("Camera control"); }
    @Override public void setKeyDown(GameKey key, boolean down) { unsupported("Game control injection"); }
    @Override public boolean keyDown(GameKey key) { return physicalKeyDown(key); }
    @Override public boolean physicalKeyDown(GameKey key) {
        int[] vk = {'W', 'S', 'A', 'D', 32, 0xA0, 0xA2, 1, 2, 'E', 'Q', 'F', 0x74, 'T', 9};
        return keys[vk[key.ordinal()]];
    }
    @Override public boolean isKeyDown(String key) {
        int vk = virtualKey(key);
        return vk >= 0 && vk < keys.length && keys[vk];
    }
    public static int virtualKey(String key) {
        return BedrockKeys.virtual(key);
    }
    @Override public String keyName(int key) {
        return BedrockKeys.name(key);
    }
    @Override public String keyLabel(String key) {
        return BedrockKeys.label(key);
    }
    @Override public boolean shiftDown() { return keys[0xA0] || keys[0xA1]; }
    @Override public boolean controlDown() { return keys[0xA2] || keys[0xA3]; }
    /** Armor rows need a complete known group and sprites from an explicitly
     * bound resource stack. Durability and main-hand inventory totals remain
     * unavailable; a pending icon never makes a worn slot appear empty. */
    @Override public List<Object[]> armor() {
        WorldFrame frame=world;WorldFrame.Player local=frame==null?null:frame.local();
        if(!worldInputAvailable||local==null||local.equipment==null||local.equipment.armor==null)return Collections.emptyList();
        List<Object[]> rows=new ArrayList<Object[]>();
        for(WorldFrame.ItemSlot slot:local.equipment.armor)if(!slot.empty) {
            if(slot.item==null||slot.count==null||itemSprites.image(slot)==null)return Collections.emptyList();
            rows.add(new Object[]{slot,null});
        }
        return rows;
    }
    @Override public List<Object[]> effects() { return Collections.emptyList(); }
    @Override public Object[] heldItem() { return null; }
    @Override public Object[] target() { return null; }
    @Override public List<Object[]> otherPlayers() {
        WorldFrame f=world;if(f==null)return Collections.emptyList();
        List<Object[]> players=new ArrayList<Object[]>();
        for(WorldFrame.Player p:f.players) if(!p.id.equals(f.localPlayer)) players.add(new Object[]{UUID.fromString(p.id),p.name,
            p.position[0],p.position[1],p.position[2],p.position[3]});
        return players;
    }
    @Override public int playerCount() { WorldFrame f=world;return f==null?-1:f.players.length; }
    // This identifies the local Arctic profile. worldPlayerId remains null until
    // an engine adapter establishes the actual player's identity in a world.
    @Override public UUID playerId() { return offlineId; }
    @Override public String playerName() { return offlineName; }
    @Override public boolean connectTo(String address) { unsupported("Joining a server"); return false; }
    @Override public void leaveWorld() { unsupported("Leaving a world"); }
    @Override public void action(MenuAction action) { unsupported("Game menu action " + action); }
    @Override public void joinServer(String id) throws Exception {
        throw new IOException("An offline profile has no Mojang session proof");
    }
    @Override public void runOnGameThread(Runnable task) { tasks.add(task); }
    @Override public void openPage(Page next) {
        if (page != null) page.removed();
        page = next;
        if (next != null) next.init(width, height);
    }
    @Override public void closePage() { openPage(null); }
    @Override public void registerCosmetic(String id, com.arcticlauncher.client.looks.Geometry geometry, byte[] png, byte[] glow) {
        try {
            com.arcticlauncher.client.looks.Cosmetics.Item item=ArcticClient.looks().cosmetics().item(id);
            AvatarRenderer.Cosmetic model=AvatarRenderer.cuboid(id,geometry,item==null?null:item.idle,
                    AvatarRenderer.png(png),glow==null?null:AvatarRenderer.png(glow));
            cosmetics.put(id,model);
            ArcticClient.looks().cosmetics().ready(id);
        } catch(IOException|IllegalArgumentException e) { log(true,"Cosmetic "+id+": "+e.getMessage()); }
    }
    @Override public void registerMesh(String id, com.arcticlauncher.client.looks.MeshModel mesh) {
        try {
            cosmetics.put(id,AvatarRenderer.mesh(id,mesh));
            ArcticClient.looks().cosmetics().ready(id);
        } catch(IOException|IllegalArgumentException e) { log(true,"Mesh "+id+": "+e.getMessage()); }
    }
    AvatarRenderer.Avatar avatar(UUID id,long timeMillis) {
        if(replayAppearanceActive()) {
            AvatarRenderer.Avatar avatar=replayAppearance.avatar(id,world);
            BodySample pose=recordedBodyPose(id);avatar.layerVisibility=pose==null?null:pose.layerVisibility();
            return avatar;
        }
        AvatarRenderer.Avatar a=new AvatarRenderer.Avatar();
        double sampleTime=world!=null&&timeMillis==world.cosmeticTimeMillis()?world.cosmeticTimeExactMillis():timeMillis;
        a.seconds=(float)((sampleTime%3_600_000)/1000d);
        if(ArcticClient.looks()==null) return a;
        com.arcticlauncher.client.looks.Looks looks=ArcticClient.looks();
        com.arcticlauncher.client.looks.Look look=looks.lookFor(id);
        a.reduceMotion=ArcticClient.config().reduceCapeMotion;
        if(look!=null) {
            a.slim=look.slim;
            if(look.skin!=null && looks.texture(look.skin)) {
                String key="look:"+look.skin;a.skinTexture(textures.get(key),javaTextureIds.get(key));
            }
            if(look.cape!=null && looks.texture(look.cape)) {
                String key="look:"+looks.frameAt(look.cape,timeMillis);a.capeTexture(textures.get(key),javaTextureIds.get(key));
            }
            for(String worn:look.cosmetics) if(looks.cosmetics().drawable(worn)!=null) {
                AvatarRenderer.Cosmetic model=cosmetics.get(worn);
                if(model!=null)a.worn.add(new AvatarRenderer.Worn(model,look.fit(worn)));
            }
        }
        com.arcticlauncher.client.looks.Cosmetics.Playing playing=looks.cosmetics().playingFor(id);
        if(playing!=null) { a.emote=playing.emote.animation;a.emoteSeconds=playing.seconds(); }
        return a;
    }
    EmotePrograms.Seed emoteSeed(UUID id) {
        if(replayAppearanceActive())return replayAppearance.emoteSeed(id,world);
        if(world==null||!worldInputAvailable||ArcticClient.looks()==null)return null;
        com.arcticlauncher.client.looks.Cosmetics.Playing playing=ArcticClient.looks().cosmetics().playingFor(id);
        if(playing==null)return null;
        com.arcticlauncher.client.looks.Animation animation=playing.emote.animation;if(animation==null)return null;
        long end=Math.min(1_000_000_000_000L,world.clockMs+250),start=Math.max(0,playing.since);
        if(end<start)return null;
        return new EmotePrograms.Seed(id.toString(),playing,animation,playing.since,
            playing.held()?playing.seconds():0,playing.held(),false,start,end);
    }
    /** A body may only be suppressed once its replacement texture is loaded.
     * Emotes also work with the observed game skin when no Arctic skin is ready. */
    static final class BodyTexture {
        final String key;final boolean slim;
        BodyTexture(String key,boolean slim){this.key=key;this.slim=slim;}
    }
    BodyTexture bodyTexture(WorldFrame.Player player,com.arcticlauncher.client.looks.Looks looks) {
        UUID id=UUID.fromString(player.id);
        if(replayAppearanceActive()) {
            if(recordedPoseMissing(id))return null;
            String skin=replayAppearance.skin(id,world);AvatarRenderer.Avatar avatar=replayAppearance.avatar(id,world);
            if(skin!=null&&texture("look:"+skin)!=null)return new BodyTexture("look:"+skin,avatar.slim);
            if(avatar.emote==null||replayAppearance.nativeSkinCaptured(id,world))return null;
        } else {
            if(looks==null)return null;
            com.arcticlauncher.client.looks.Look look=looks.lookFor(id);
            if(look!=null && look.skin!=null && look.skin.matches("[0-9a-f]{40}")
                    && textures.containsKey("look:"+look.skin))return new BodyTexture("look:"+look.skin,look.slim);
            if(looks.cosmetics().playingFor(id)==null)return null;
        }
        // nativeSkin survives a custom-body draw; player.skin identifies the
        // texture that was actually used for that particular replacement.
        String nativeSkin=player.nativeSkin!=null?player.nativeSkin:player.skin;
        boolean slim=player.nativeSlim!=null?player.nativeSlim:player.slim;
        if(nativeSkin==null || !nativeSkin.matches("engine:skin/[0-9a-f]{64}")
                || !textures.containsKey(nativeSkin))return null;
        return new BodyTexture(nativeSkin,slim);
    }
    static final class CapeTexture {
        final String hash,key,nativeCape;
        CapeTexture(String hash,String key,String nativeCape){this.hash=hash;this.key=key;this.nativeCape=nativeCape;}
    }
    CapeTexture capeTexture(WorldFrame.Player player,long clockMs,com.arcticlauncher.client.looks.Looks looks) {
        // Matching raw native art stays in its original native material/mesh;
        // its recorded palette supplies motion instead of the Java cape mesh.
        if(nativeCapeReplayRequested(player))return null;
        if(recordedPoseMissing(UUID.fromString(player.id)))return null;
        if(replayAppearanceActive()&&replayAppearance.hasLayerColors()&&!replayAppearance.capeColorReady(UUID.fromString(player.id),world))return null;
        if(player.capeMotion==null || !Boolean.TRUE.equals(player.chestEmpty))return null;
        String nativeCape=player.nativeCape!=null?player.nativeCape:player.cape;
        if(nativeCape==null || !nativeCape.matches("engine:cape/[0-9a-f]{64}"))return null;
        String hash;
        if(replayAppearanceActive())hash=replayAppearance.cape(UUID.fromString(player.id),world);
        else {
            if(looks==null)return null;
            com.arcticlauncher.client.looks.Look look=looks.lookFor(UUID.fromString(player.id));
            if(look==null||look.cape==null||!looks.texture(look.cape))return null;
            hash=look.cape;
        }
        if(hash==null||!hash.matches("[0-9a-f]{40}"))return null;
        if(recordedCapeMotionActive()&&capeMotion(UUID.fromString(player.id),player.capeMotion)==null)return null;
        String key=replayAppearanceActive()?replayAppearance.capeTexture(UUID.fromString(player.id),world):capeFrame(hash,clockMs);
        if(key==null||texture(key)==null)return null;
        return new CapeTexture(hash,key,nativeCape);
    }
    boolean nativeCapeReplayRequested(WorldFrame.Player player) {
        return replayAppearanceActive()&&replayAppearance.hasNativeCapePaletteTrack()
            &&replayAppearance.usesNativeCape(UUID.fromString(player.id),world);
    }
    String nativeCapeReplayIdentity(WorldFrame.Player player) {
        return nativeCapeReplayRequested(player)?replayAppearance.nativeCapeIdentity(UUID.fromString(player.id),world):null;
    }
    float[] nativeCapeReplayColor(WorldFrame.Player player,String geometry) {
        String identity=nativeCapeReplayIdentity(player);if(identity==null)return null;
        LayerColorSample sample=recordedLayerColors(UUID.fromString(player.id));if(sample==null)return null;
        try {return sample.colorFor(LayerColorSample.CAPE,geometry,identity);}
        catch(IOException missing){return null;}
    }
    Boolean nativeCapeReplayDithering(WorldFrame.Player player,String geometry) {
        String identity=nativeCapeReplayIdentity(player);if(identity==null)return null;
        LayerColorSample sample=recordedLayerColors(UUID.fromString(player.id));if(sample==null)return null;
        try {return sample.ditheringFor(LayerColorSample.CAPE,geometry,identity);}
        catch(IOException missing){return null;}
    }
    ActorShaderInputs nativeCapeReplayShaderInputs(WorldFrame.Player player,String geometry) {
        LayerColorSample sample=recordedLayerColors(UUID.fromString(player.id));if(sample==null)return null;
        String key=nativeCapeReplayIdentity(player);if(key==null)return null;
        try{return sample.shaderInputsFor(LayerColorSample.CAPE,geometry,key);}catch(IOException failure){return null;}
    }
    boolean nativeWingReplayRequested(WorldFrame.Player player) {
        return replayAppearanceActive()&&replayAppearance.hasNativeWingPaletteTrack()
            &&Boolean.TRUE.equals(player.elytraEquipped)&&!player.replaceElytra;
    }
    private LayerColorSample nativeWingReplayColors(WorldFrame.Player player,String geometry) {
        return nativeWingReplayRequested(player)&&"geometry.elytra".equals(geometry)
            ?recordedLayerColors(UUID.fromString(player.id)):null;
    }
    float[] nativeWingReplayColor(WorldFrame.Player player,String geometry) {
        LayerColorSample sample=nativeWingReplayColors(player,geometry);if(sample==null)return null;
        try{return sample.colorFor(LayerColorSample.WING,geometry,null);}catch(IOException missing){return null;}
    }
    Boolean nativeWingReplayDithering(WorldFrame.Player player,String geometry) {
        LayerColorSample sample=nativeWingReplayColors(player,geometry);if(sample==null)return null;
        try{return sample.ditheringFor(LayerColorSample.WING,geometry,null);}catch(IOException missing){return null;}
    }
    ActorShaderInputs nativeWingReplayShaderInputs(WorldFrame.Player player,String geometry) {
        LayerColorSample sample=nativeWingReplayColors(player,geometry);if(sample==null)return null;
        try{return sample.shaderInputsFor(LayerColorSample.WING,geometry,null);}catch(IOException missing){return null;}
    }
    boolean nativeCapeReplayReady(WorldFrame.Player player) {
        if(!nativeCapeReplayRequested(player)||recordedPoseMissing(UUID.fromString(player.id))||player.nativeCape==null
                ||player.nativeCapeSuppressed||player.nativeCapeHidden||player.replaceCape||player.replaceElytra
                ||player.nativeCapeAbsent||!Boolean.TRUE.equals(player.chestEmpty)
                ||!player.nativeCape.equals(nativeCapeReplayIdentity(player)))return false;
        AttachmentSample expected=recordedAttachmentPose(UUID.fromString(player.id));
        if(expected==null||!expected.nativeCapeReplayed(player.attachmentPoses,player.nativeCape,
                replayAppearanceManifestHash(),world.replayCosmeticSample()))return false;
        WorldFrame.CapeReplayColor color=player.nativeCapeReplayColor;if(color==null)return false;
        float[] rgba=nativeCapeReplayColor(player,color.geometry);
        Boolean dithering=nativeCapeReplayDithering(player,color.geometry);
        ActorShaderInputs shaderInputs=nativeCapeReplayShaderInputs(player,color.geometry);
        return rgba!=null&&Arrays.equals(rgba,color.rgba)&&dithering!=null&&dithering.equals(color.ditheringEnabled)
            &&shaderInputs!=null&&shaderInputs.hasMaterialContext()&&shaderInputs.hasMaterialPaths()&&shaderInputs.hasMaterialVariation()&&ActorShaderInputs.same(shaderInputs,color.shaderInputs)
            &&Boolean.TRUE.equals(color.shaderInputsApplied)&&color.shaderInputDraws>0&&color.shaderInputCopies==color.shaderInputDraws
            &&color.replay.matches(replayAppearanceManifestHash(),world.replayCosmeticSample());
    }
    /** Explicit recorded absence can hide an observed native cape. Unknown
     * archive state never authorizes this operation. */
    String hiddenCape(WorldFrame.Player player) {
        UUID id=UUID.fromString(player.id);
        if(!replayAppearanceActive()||!replayAppearance.nativeCapeAbsent(id,world)||recordedPoseMissing(id)
                ||!Boolean.TRUE.equals(player.chestEmpty))return null;
        return player.nativeCape!=null&&player.nativeCape.matches("engine:cape/[0-9a-f]{64}")?player.nativeCape:null;
    }
    boolean replayAppearanceActive() {return replayAppearance!=null&&world!=null&&world.session.equals(replayAppearanceSession);}
    String replayAppearanceManifestHash(){return replayAppearanceActive()?replayAppearance.manifestHash():null;}
    static final class ElytraTexture {
        final String hash,key;
        ElytraTexture(String hash,String key){this.hash=hash;this.key=key;}
    }
    ElytraTexture elytraTexture(WorldFrame.Player player,long clockMs,com.arcticlauncher.client.looks.Looks looks) {
        if(recordedPoseMissing(UUID.fromString(player.id)))return null;
        if(replayAppearanceActive()&&replayAppearance.hasLayerColors()&&!replayAppearance.wingColorReady(UUID.fromString(player.id),world))return null;
        if(!Boolean.TRUE.equals(player.elytraEquipped)||player.wingMatrices==null)return null;
        if(replayAppearanceActive()&&recordedWingPose(UUID.fromString(player.id))==null)return null;
        if(recordedJavaWings()&&recordedJavaWingMotion(UUID.fromString(player.id))==null)return null;
        String hash;
        if(replayAppearanceActive())hash=replayAppearance.wingCape(UUID.fromString(player.id),world);
        else {
            if(looks==null)return null;
            com.arcticlauncher.client.looks.Look look=looks.lookFor(UUID.fromString(player.id));
            if(look==null||look.cape==null||!looks.texture(look.cape))return null;
            hash=look.cape;
        }
        if(hash==null||!hash.matches("[0-9a-f]{40}"))return null;
        String key=capeFrame(hash,clockMs);
        if(key==null||texture(key)==null)return null;
        return new ElytraTexture(hash,key);
    }
    boolean recordedCapeMotionActive() {return replayAppearanceActive()&&replayAppearance.hasCapeHistory();}
    BodySample recordedBodyPose(UUID id) {return replayAppearanceActive()?replayAppearance.bodyPose(id,world):null;}
    boolean recordedModelRoots(){return replayAppearanceActive()&&replayAppearance.hasModelRoots();}
    float[] recordedModelRoot(UUID id){return replayAppearanceActive()?replayAppearance.modelRoot(id,world):null;}
    private boolean recordedPoseMissing(UUID id) {
        if(!replayAppearanceActive()||!replayAppearance.hasBodyHistory())return false;
        BodySample pose=recordedBodyPose(id);
        return pose==null||replayAppearance.hasModelRoots()&&recordedModelRoot(id)==null
            ||replayAppearance.hasBodyColors()&&!pose.hasColor()
            ||replayAppearance.hasBodyPackedLight()&&!pose.hasPackedLight()
            ||replayAppearance.hasBodySkinMaterials()&&!pose.hasSkinMaterial()
            ||replayAppearance.hasBodySkinVisibility()&&!pose.hasSkinVisibility()
            ||replayAppearance.hasBodyLayerVisibility()&&!pose.hasLayerVisibility();
    }
    AttachmentSample recordedAttachmentPose(UUID id) {return replayAppearanceActive()?replayAppearance.attachmentPose(id,world):null;}
    boolean recordedEquipmentMatches(UUID id,WorldFrame.Equipment actual) {
        return !replayAppearanceActive()||replayAppearance.equipmentMatches(id,world,actual);
    }
    boolean recordedSkinReady(UUID id) {return !replayAppearanceActive()||replayAppearance.skinReady(id,world);}
    boolean recordedCapeReady(UUID id) {return !replayAppearanceActive()||replayAppearance.capeReady(id,world);}
    boolean recordedCapeColorReady(UUID id) {return !replayAppearanceActive()||replayAppearance.capeColorReady(id,world);}
    LayerColorSample recordedLayerColors(UUID id) {return replayAppearanceActive()?replayAppearance.layerColors(id,world):null;}
    JavaEntityShader recordedEntityShader(WorldFrame observation) {return replayAppearanceActive()?replayAppearance.entityShader(observation):null;}
    GlintSample recordedWingGlint(UUID id,WorldFrame observation) {
        return replayAppearanceActive()?replayAppearance.wingGlint(id,observation):null;
    }
    boolean recordedCapeAbsent(UUID id) {return replayAppearanceActive()&&replayAppearance.nativeCapeAbsent(id,world);}
    CapePhysics.Motion capeMotion(UUID id,CapePhysics.Motion nativeMotion) {
        return recordedCapeMotionActive()?replayAppearance.capeMotion(id,world):nativeMotion;
    }
    WingSample recordedWingPose(UUID id) {
        return replayAppearanceActive()?replayAppearance.wingPose(id,world):null;
    }
    boolean recordedJavaWings(){return replayAppearanceActive()&&replayAppearance.hasJavaWingHistory();}
    JavaWingPhysics.Motion recordedJavaWingMotion(UUID id) {
        return replayAppearanceActive()?replayAppearance.javaWingMotion(id,world):null;
    }
    /** Disconnect/session reset discards history; explicit unmount retains pixels. */
    void clearReplayAppearance() {
        replayAppearance=null;replayAppearanceSession=null;replayAppearanceOwner=null;
        retiredReplayTextures.clear();retiredReplayTextureIds.clear();retiredReplayBytes=0;
    }
    private void checkAppearanceOwner(String owner) throws IOException {
        if(replayAppearanceActive()&&!Objects.equals(owner,replayAppearanceOwner))throw new IOException("recorded appearance belongs to another control owner");
    }
    private void retainReplayTextures() throws IOException {
        if(!replayAppearanceActive())return;
        long added=0;
        for(Map.Entry<String,BufferedImage> entry:replayAppearance.textures.entrySet())if(!retiredReplayTextures.containsKey(entry.getKey())) {
            BufferedImage image=entry.getValue();added+=4L*image.getWidth()*image.getHeight();
        }
        if(retiredReplayBytes+added>64L*1024*1024)throw new IOException("appearance transition exceeds the 64 MiB retained texture budget; reconnect before changing more snapshots");
        // Preflight completes before mutation. Never evict pixels that a
        // buffered native draw may still need, or partially retire a mount.
        for(Map.Entry<String,BufferedImage> entry:replayAppearance.textures.entrySet())if(!retiredReplayTextures.containsKey(entry.getKey())) {
            retiredReplayTextures.put(entry.getKey(),entry.getValue());
            String id=replayAppearance.textureId(entry.getKey());if(id!=null)retiredReplayTextureIds.put(entry.getKey(),id);
        }
        retiredReplayBytes+=added;
    }
    private void checkReplayTextureBudget(ReplayAppearance prepared) throws IOException {
        Set<String> keys=new HashSet<String>(retiredReplayTextures.keySet());long bytes=retiredReplayBytes;
        ReplayAppearance[] appearances={replayAppearanceActive()?replayAppearance:null,prepared};
        for(ReplayAppearance appearance:appearances)if(appearance!=null)
            for(Map.Entry<String,BufferedImage> entry:appearance.textures.entrySet())if(keys.add(entry.getKey())) {
                BufferedImage image=entry.getValue();bytes+=4L*image.getWidth()*image.getHeight();
            }
        // Reserve eventual clear as part of mount admission, so its owner can
        // always release an accepted snapshot without exceeding this budget.
        if(bytes>64L*1024*1024)throw new IOException("appearance history exceeds the 64 MiB texture budget; reconnect before changing more snapshots");
    }
    void clearReplayAppearance(String owner) throws IOException {
        checkAppearanceOwner(owner);retainReplayTextures();
        replayAppearance=null;replayAppearanceSession=null;replayAppearanceOwner=null;
    }
    void mountReplayAppearance(File file,String session,String owner) throws IOException {
        if(world==null||!worldInputAvailable||!world.session.equals(session))throw new IOException("appearance mount belongs to a different or absent native world");
        if(appearanceHistory!=null&&appearanceHistory.busy())throw new IOException("finish appearance history before mounting recorded looks");
        checkAppearanceOwner(owner);
        long started=System.nanoTime();
        ReplayAppearance prepared=ReplayAppearance.load(file);prepared.validateViewer(world);
        try {prepared.time(world);} catch(IllegalArgumentException invalidTime) {throw new IOException(invalidTime.getMessage(),invalidTime);}
        if(System.nanoTime()-started>20_000_000_000L)throw new IOException("appearance preparation expired before commit");
        checkReplayTextureBudget(prepared);retainReplayTextures();
        // Preparation is isolated from live texture/model maps. A failure above
        // preserves the current appearance and its acknowledged body plan.
        replayAppearance=prepared;replayAppearanceSession=session;replayAppearanceOwner=owner;
    }
    void replayAppearanceTime(String session,long time,String owner) throws IOException {
        if(!replayAppearanceActive()||!world.session.equals(session))throw new IOException("appearance time belongs to a different native world");
        checkAppearanceOwner(owner);
        Long dispatched=world.replayCosmeticTimeMillis();
        if(dispatched!=null&&dispatched!=time)throw new IOException("appearance time differs from the active native replay marker");
        replayAppearance.setTime(time);
    }
    void saveReplayAppearance(File file) throws IOException {replayAppearance.save(appearanceAssets,world,file);}
    BufferedImage texture(String key) {
        BufferedImage saved=replayAppearanceActive()?replayAppearance.textures.get(key):null;
        if(saved==null)saved=retiredReplayTextures.get(key);
        return saved==null?textures.get(key):saved;
    }
    /** Match texture()'s selected owner, including unknown native-only images.
     * A lower-priority live registration must not name an archived binding. */
    String textureId(String key) {
        if(key==null)return null;
        if(replayAppearanceActive()&&replayAppearance.textures.containsKey(key))return replayAppearance.textureId(key);
        if(retiredReplayTextures.containsKey(key))return retiredReplayTextureIds.get(key);
        return textures.containsKey(key)?javaTextureIds.get(key):null;
    }
    String capeFrame(String hash,long time) {
        if(replayAppearanceActive())return replayAppearance.capeFrame(hash,replayAppearance.time(world));
        com.arcticlauncher.client.looks.Looks looks=ArcticClient.looks();
        return looks==null?null:"look:"+looks.frameAt(hash,time);
    }
    com.google.gson.JsonObject replayAppearanceStatus() {
        if(!replayAppearanceActive())return null;
        com.google.gson.JsonObject state=replayAppearance.status(world);state.addProperty("owner",replayAppearanceOwner);return state;
    }
    private com.arcticlauncher.client.looks.AppearanceSnapshot.Captured captureHistory(long time) throws IOException {
        if(historySkins==null)throw new IOException("native skin history was not prepared");
        return captureAppearance(time,true,historySkins);
    }
    void saveNativeAppearance(File output) throws IOException {
        com.arcticlauncher.client.looks.AppearanceSnapshot.Captured captured=captureAppearance(world.cosmeticTimeMillis(),false,new NativeSkinCapture(appearanceAssets));
        appearanceAssets.archive(output,captured.state,captured.files);
    }
    private com.arcticlauncher.client.looks.AppearanceSnapshot.Captured captureAppearance(long time,boolean drawableOnly,NativeSkinCapture skins) throws IOException {
        List<UUID> players=new ArrayList<UUID>();for(WorldFrame.Player player:world.players)players.add(UUID.fromString(player.id));
        Map<UUID,com.google.gson.JsonObject> equipment=new HashMap<UUID,com.google.gson.JsonObject>();
        for(WorldFrame.Player player:world.players)if(player.equipment!=null)
            equipment.put(UUID.fromString(player.id),NATIVE_STATE.toJsonTree(player.equipment).getAsJsonObject());
        com.arcticlauncher.client.looks.AppearanceSnapshot.Captured captured=com.arcticlauncher.client.looks.AppearanceSnapshot.capture(ArcticClient.looks(),players,
            world.localPlayer==null?null:UUID.fromString(world.localPlayer),world.session,time,world.cosmeticTimeMillis(),ArcticClient.config().reduceCapeMotion,drawableOnly);
        captured=equipmentCapture.attach(captured,equipment,appearanceAssets);
        return skins.attach(captured,world,this);
    }
    void beginAppearanceHistory(File file,String session,long origin,String owner) throws IOException {
        beginAppearanceHistory(file,session,origin,owner,null);
    }
    void beginAnchoredAppearanceHistory(File file,String session,long relayTime,String owner,com.google.gson.JsonObject anchor) throws IOException {
        try {
            if(world==null||world.nativeTiming==null||!session.equals(anchor.get("session").getAsString())
                    ||anchor.get("relayTimeMs").getAsLong()!=relayTime||!"client_tick_completed".equals(anchor.get("stage").getAsString())
                    ||!anchor.get("dispatcherReturned").getAsBoolean()||!anchor.get("primaryClientThread").getAsBoolean()
                    ||!anchor.get("postClientTickCompleted").getAsBoolean())throw new IOException("recording anchor has no matching completed native tick");
            CapeHistory.clock(world);
            long tick=anchor.get("clientTick").getAsLong();
            double at=anchor.get("clientTickElapsedMs").getAsDouble();
            double elapsed=(world.nativeTiming.get("elapsedMs").getAsDouble()-at)
                +(double)world.nativeTiming.get("partial").getAsFloat()/world.nativeTiming.get("ticksPerSecond").getAsFloat()*1000;
            if(!Double.isFinite(at)||at<0||!Double.isFinite(elapsed)||elapsed<0||elapsed>3000||world.nativeTiming.get("ticks").getAsLong()<tick)
                throw new IOException("recording anchor is newer than the world frame or more than three seconds old");
            long mapped=relayTime+(long)Math.floor(elapsed);
            com.google.gson.JsonObject association=new com.google.gson.JsonParser().parse(anchor.toString()).getAsJsonObject();
            association.addProperty("mappedStartTimeMs",mapped);association.addProperty("alignmentVerified",false);
            com.google.gson.JsonObject shaderClock=NativePresentationClock.associate(world,association,relayTime);
            association.addProperty("presentationClockMappingAvailable",shaderClock!=null);
            if(shaderClock!=null)association.add("shaderClockAssociation",shaderClock);
            beginAppearanceHistory(file,session,mapped,owner,association);
        } catch(RuntimeException invalid) {throw new IOException("invalid native recording anchor",invalid);}
    }
    private void beginAppearanceHistory(File file,String session,long origin,String owner,com.google.gson.JsonObject association) throws IOException {
        if(world==null||!worldInputAvailable||!world.session.equals(session)||origin<0||origin>86_400_000
                ||owner==null||!owner.matches("[A-Za-z0-9_-]{1,64}"))throw new IOException("appearance history needs a current world, bounded origin and owner");
        if(replayAppearanceActive()||world.replayCosmeticTimeMillis()!=null||appearanceHistory!=null&&appearanceHistory.busy())throw new IOException("another appearance playback/recording owns this client");
        historySkins=new NativeSkinCapture(appearanceAssets);
        com.arcticlauncher.client.looks.AppearanceSnapshot.Captured first=captureHistory(origin);
        appearanceHistory=new com.arcticlauncher.client.looks.AppearanceHistory(appearanceAssets,config,file,association);
        historySession=session;historyOwner=owner;historyClock=world.clockMs;historyOrigin=origin;
        try {
            capeHistory=new CapeHistory.Writer(appearanceHistory,world,origin);
            wingHistory=new WingHistory.Writer(appearanceHistory);
            bodyHistory=new BodyHistory.Writer(appearanceHistory);
            attachmentHistory=new AttachmentHistory.Writer(appearanceHistory);
            layerColorHistory=new LayerColorHistory.Writer(appearanceHistory);
            entityMaterialHistory=new EntityMaterialHistory.Writer(appearanceHistory);
            glintHistory=new GlintHistory.Writer(appearanceHistory);
            shaderClockHistory=new ShaderClockHistory.Writer(appearanceHistory,association,session);
            shaderClockHistory.observe(world);
            appearanceHistory.accept(first);capeHistory.observe(world);wingHistory.observe(world,origin);bodyHistory.observe(world,origin);
            attachmentHistory.observe(world,origin);
            layerColorHistory.observe(world,origin);
            entityMaterialHistory.observe(world,origin);
            glintHistory.observe(world,origin);
        } catch(IOException|RuntimeException failure) {appearanceHistory.cancel(failure.toString());throw failure;}
    }
    private long historyTime() throws IOException {
        if(!worldInputAvailable)throw new IOException("appearance history needs a current world frame");
        if(world==null||!world.session.equals(historySession)||world.clockMs<historyClock||world.replayCosmeticTimeMillis()!=null)throw new IOException("appearance recording world or clock changed");
        double elapsed=capeHistory.elapsed(world);
        if(elapsed<0||elapsed>86_400_000-historyOrigin)throw new IOException("appearance history exceeds its clock range");
        return historyOrigin+(long)Math.floor(elapsed);
    }
    void sampleAppearanceHistory() {
        if(appearanceHistory==null||!appearanceHistory.recording()||!worldInputAvailable)return;
        try {
            shaderClockHistory.observe(world);
            appearanceHistory.accept(captureHistory(historyTime()));capeHistory.observe(world);
            wingHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
            bodyHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
            attachmentHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
            layerColorHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
            entityMaterialHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
            glintHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
        }
        catch(IOException|RuntimeException failure) {appearanceHistory.cancel(failure.toString());log(true,"Appearance history: "+failure.getMessage());}
    }
    void finishAppearanceHistory(String session,String owner,boolean cancel) throws IOException {
        if(appearanceHistory==null||!Objects.equals(historySession,session)||!Objects.equals(historyOwner,owner))throw new IOException("appearance history belongs to another world or owner");
        if(cancel) {appearanceHistory.cancel("appearance history cancelled by its owner");return;}
        if(appearanceHistory.recording()) {
            long time=historyTime();
            try {
                shaderClockHistory.observe(world);
                appearanceHistory.accept(captureHistory(time));capeHistory.observe(world);
                wingHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
                bodyHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
                attachmentHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
                layerColorHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
                entityMaterialHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
                glintHistory.observe(world,historyOrigin+capeHistory.elapsed(world));
                capeHistory.finish();wingHistory.finish();bodyHistory.finish();attachmentHistory.finish();layerColorHistory.finish();entityMaterialHistory.finish();glintHistory.finish();shaderClockHistory.finish();appearanceHistory.finish(time);
            }
            catch(IOException|RuntimeException failure) {appearanceHistory.cancel(failure.toString());throw failure;}
        }
        if("failed".equals(appearanceHistory.status().get("stage").getAsString()))throw new IOException("appearance history failed: "+appearanceHistory.status().get("error").getAsString());
    }
    void appearanceHistoryWorldEnded() {
        if(appearanceHistory!=null)appearanceHistory.cancel("native world ended before appearance history finished");
    }
    com.google.gson.JsonObject appearanceHistoryStatus() {
        if(appearanceHistory==null)return null;
        com.google.gson.JsonObject state=appearanceHistory.status();state.addProperty("session",historySession);state.addProperty("owner",historyOwner);
        if(shaderClockHistory!=null)state.add("shaderClock",shaderClockHistory.status());return state;
    }
    @Override public void registerTexture(String hash, byte[] png, boolean cape) {
        try {
            BufferedImage image = ImageIO.read(new ByteArrayInputStream(png));
            if (image == null) throw new IOException("invalid PNG");
            int frames = cape ? com.arcticlauncher.client.looks.Looks.capeFrames(image.getWidth(), image.getHeight()) : 1;
            if (cape && frames > 1) {
                int h = image.getHeight() / frames;
                javaTextureIds.remove("look:"+hash);
                for (int i = 0; i < frames; i++) {
                    String path=hash+"/"+i,key="look:"+path;
                    javaTextureIds.put(key,JavaTextureId.look(path));
                    textures.put(key,image.getSubimage(0, i * h, image.getWidth(), h));
                }
            } else {
                javaTextureIds.put("look:"+hash,JavaTextureId.look(hash));
            }
            // Keep the strip for existing callers; Fabric registers only the
            // individual frames when animated, so the strip has no Java ID.
            textures.put("look:" + hash, image);
            ArcticClient.looks().textureReady(hash, frames);
        } catch (IOException e) { log(true, "Texture: " + e.getMessage()); }
    }
    private void unsupported(String feature) {
        com.arcticlauncher.client.notice.Notices.post(Lang.t("platform.bedrock_binding_pending"), feature + " needs a verified engine adapter."); // i18n-ignore
    }
}
