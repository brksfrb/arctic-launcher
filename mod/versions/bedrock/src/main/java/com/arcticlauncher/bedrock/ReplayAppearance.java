package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.*;
import java.awt.image.BufferedImage;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import javax.imageio.ImageIO;
import javax.imageio.ImageReader;
import javax.imageio.stream.ImageInputStream;

import com.arcticlauncher.client.Lang;
/** Prepared recorded appearance. Loading does not query Looks, change its
 * catalog, or touch native components. Commit happens only after all parsing. */
final class ReplayAppearance {
    private static final Gson GSON=new Gson();
    private static final long MAX_RESIDENT=128L*1024*1024;
    private final File source;
    private final String manifestHash;
    private final JsonObject manifest;
    private final Snapshot snapshot;
    private final Map<String,Actor> actors=new HashMap<String,Actor>();
    private final Map<String,AvatarRenderer.Cosmetic> models=new HashMap<String,AvatarRenderer.Cosmetic>();
    private final Map<String,Animation> moves=new HashMap<String,Animation>();
    final Map<String,BufferedImage> textures;
    private final Library library;
    private final Map<String,Integer> rates=new HashMap<String,Integer>();
    private NavigableMap<Long,ReplayAppearance> timeline;
    private CapeHistory.Reader capeHistory;
    private WingHistory.Reader wingHistory;
    private BodyHistory.Reader bodyHistory;
    private AttachmentHistory.Reader attachmentHistory;
    private LayerColorHistory.Reader layerColorHistory;
    private EntityMaterialHistory.Reader entityMaterialHistory;
    private GlintHistory.Reader glintHistory;
    private ShaderClockHistory.Reader shaderClockHistory;
    private String capeHistoryError;
    private String wingHistoryError;
    private String bodyHistoryError;
    private String attachmentHistoryError;
    private String layerColorHistoryError;
    private String entityMaterialHistoryError;
    private String glintHistoryError;
    private String equipmentHistoryError;
    private JsonObject association;
    private long heldTime,endTime=86_400_000;
    /** Shared immutable assets across every event, with one preparation budget. */
    private static final class Library {
        long resident;
        final Map<String,BufferedImage> textures=new HashMap<String,BufferedImage>(),modelTextures=new HashMap<String,BufferedImage>();
        final Map<String,Integer> frames=new HashMap<String,Integer>();
        final Map<String,String> nativeCapeIdentities=new HashMap<String,String>();
        final Map<String,String> javaTextureIds=new HashMap<String,String>();
        final Map<String,AvatarRenderer.Cosmetic> models=new HashMap<String,AvatarRenderer.Cosmetic>();
        final Map<String,Animation> animations=new HashMap<String,Animation>();
        final Map<String,WorldFrame.Equipment> equipment=new HashMap<String,WorldFrame.Equipment>();
    }

    private static final class Snapshot {
        int format,rig;long timeMs;Long cosmeticTimeMs;String session,localPlayer;boolean reduceMotion;
        Actor[] players;Model[] cosmetics;Move[] emotes;
    }
    private static final class Actor {
        String id;LookAt look;EmoteAt emote;WorldFrame.Equipment bedrockEquipment;NativeSkinAt bedrockSkin;NativeCapeAt bedrockCape;
        EquipmentSource bedrockEquipmentSource;
        transient boolean nativeSkinCaptured,nativeCapeCaptured;
    }
    private static final class NativeSkinAt {String texture;boolean slim;}
    private static final class EquipmentSource {int schema;String hash;}
    private static final class NativeCapeAt {boolean present;String texture;}
    private static final class LookAt {String skin,cape;boolean slim,arctic;int capeFps;String[] cosmetics;Map<String,FitAt> fits;}
    private static final class FitAt {int up,forward;boolean mirror;}
    private static final class EmoteAt {String id;float seconds;boolean held;Double elapsedMillis;}
    private static final class Model {String id,name,slot,mesh,model,texture,glow,animation;}
    private static final class Move {String id,name,animation;}

    static ReplayAppearance load(File file) throws IOException {
        try(AppearanceAssets.Archive archive=new AppearanceAssets.Archive(file)) {
            JsonObject header=archive.manifest();boolean history=header.get("format").getAsInt()==2;
            JsonArray changes=null;
            if(history) {
                whole(header,"timeMs",0,86_400_000);whole(header,"durationMs",0,86_400_000);
                JsonElement events=header.get("events");
                if(events==null||!events.isJsonArray()||events.getAsJsonArray().size()<1||events.getAsJsonArray().size()>4096)
                    throw new IOException("invalid recorded appearance history index");
                changes=events.getAsJsonArray();
            }
            Library library=new Library();
            ReplayAppearance result=new ReplayAppearance(file,archive,history?historyState(archive,changes.get(0),library):header,library);
            // Verify even unused index entries before mounting. Reads stay
            // bounded to one raw file; no extraction or bulk asset allocation.
            for(AppearanceAssets.Ref ref:archive.references())archive.read(ref.kind,ref.hash);
            result.prepare(archive);
            if(history) {
                if(!header.get("session").equals(result.manifest.get("session"))||!header.get("localPlayer").equals(result.manifest.get("localPlayer"))
                        ||header.get("timeMs").getAsLong()!=result.snapshot.timeMs)throw new IOException("appearance history origin differs from its initial state");
                result.endTime=header.get("durationMs").getAsLong();result.timeline=new TreeMap<Long,ReplayAppearance>();
                result.timeline.put(result.snapshot.timeMs,result);long previous=result.snapshot.timeMs;
                for(int i=1;i<changes.size();i++) {
                    ReplayAppearance event=new ReplayAppearance(file,archive,historyState(archive,changes.get(i),library),library);
                    if(event.snapshot.timeMs<=previous||!event.snapshot.session.equals(result.snapshot.session)
                            ||!Objects.equals(event.snapshot.localPlayer,result.snapshot.localPlayer))throw new IOException("appearance history changes need ordered times and one world/local identity");
                    event.prepare(archive);result.timeline.put(event.snapshot.timeMs,event);previous=event.snapshot.timeMs;
                }
                if(result.endTime<previous)throw new IOException("appearance history ends before its last change");
            }
            JsonElement tracks=header.get("tracks");
            JsonElement source=header.get("source");
            if(source!=null) {
                if(!history||!source.isJsonObject())throw new IOException("recording association requires an appearance history");
                JsonObject data=source.getAsJsonObject();String id=data.get("recordingId").getAsString();
                if(!UUID.fromString(id).toString().equals(id)||!data.get("session").getAsString().equals(result.snapshot.session)
                        ||!"marker_post_tick".equals(data.get("alignment").getAsString()))throw new IOException("invalid appearance recording association");
                NativePresentationClock.validateAssociation(data);
                result.association=data;
            }
            if(tracks!=null) {
                if(!history||!tracks.isJsonObject())throw new IOException("appearance tracks require a history archive");
                for(Map.Entry<String,JsonElement> track:tracks.getAsJsonObject().entrySet()) {
                    if(!track.getValue().isJsonArray())throw new IOException("unsupported appearance adapter track");
                    if(CapeHistory.KIND.equals(track.getKey())||CapeHistory.LEGACY_KIND.equals(track.getKey())) {
                        if(result.capeHistory!=null)throw new IOException("appearance archive has conflicting cape tick tracks");
                        int version=CapeHistory.KIND.equals(track.getKey())?2:1;
                        result.reserve((version>=2?16L:8L)*1024*1024);result.capeHistory=new CapeHistory.Reader(file,archive,track.getValue().getAsJsonArray(),version);
                    } else if(WingHistory.KIND.equals(track.getKey())) {
                        result.reserve(16L*1024*1024);result.wingHistory=new WingHistory.Reader(file,archive,track.getValue().getAsJsonArray());
                    } else if(BodyHistory.KIND.equals(track.getKey())||BodyHistory.VISIBILITY_KIND.equals(track.getKey())||BodyHistory.MATERIALS_KIND.equals(track.getKey())||BodyHistory.LIGHTS_KIND.equals(track.getKey())||BodyHistory.COLORS_KIND.equals(track.getKey())||BodyHistory.ROOTS_KIND.equals(track.getKey())||BodyHistory.LEGACY_KIND.equals(track.getKey())) {
                        if(result.bodyHistory!=null)throw new IOException("appearance archive has conflicting body tracks");
                        int version=BodyHistory.KIND.equals(track.getKey())?7:BodyHistory.VISIBILITY_KIND.equals(track.getKey())?6:BodyHistory.MATERIALS_KIND.equals(track.getKey())?5:BodyHistory.LIGHTS_KIND.equals(track.getKey())?4:BodyHistory.COLORS_KIND.equals(track.getKey())?3:BodyHistory.ROOTS_KIND.equals(track.getKey())?2:1;
                        result.reserve(16L*1024*1024);result.bodyHistory=new BodyHistory.Reader(file,archive,track.getValue().getAsJsonArray(),version);
                    } else if(AttachmentHistory.KIND.equals(track.getKey())||AttachmentHistory.CAPE_KIND.equals(track.getKey())||AttachmentHistory.LEGACY_KIND.equals(track.getKey())) {
                        if(result.attachmentHistory!=null)throw new IOException("appearance archive has conflicting attachment tracks");
                        int version=AttachmentHistory.KIND.equals(track.getKey())?3:AttachmentHistory.CAPE_KIND.equals(track.getKey())?2:1;
                        result.reserve(16L*1024*1024);result.attachmentHistory=new AttachmentHistory.Reader(file,archive,track.getValue().getAsJsonArray(),version);
                    } else if(LayerColorHistory.KIND.equals(track.getKey())||LayerColorHistory.PATHS_KIND.equals(track.getKey())||LayerColorHistory.CONTEXT_KIND.equals(track.getKey())||LayerColorHistory.SHADER_KIND.equals(track.getKey())||LayerColorHistory.DITHERING_KIND.equals(track.getKey())||LayerColorHistory.LEGACY_KIND.equals(track.getKey())) {
                        if(result.layerColorHistory!=null)throw new IOException("appearance archive has conflicting layer color tracks");
                        int version=LayerColorHistory.KIND.equals(track.getKey())?6:LayerColorHistory.PATHS_KIND.equals(track.getKey())?5:LayerColorHistory.CONTEXT_KIND.equals(track.getKey())?4:LayerColorHistory.SHADER_KIND.equals(track.getKey())?3:LayerColorHistory.DITHERING_KIND.equals(track.getKey())?2:1;
                        result.reserve(8L*1024*1024);result.layerColorHistory=new LayerColorHistory.Reader(file,archive,track.getValue().getAsJsonArray(),version);
                    } else if(EntityMaterialHistory.KIND.equals(track.getKey())) {
                        if(result.entityMaterialHistory!=null)throw new IOException("appearance archive has conflicting entity material tracks");
                        result.reserve(8L*1024*1024);result.entityMaterialHistory=new EntityMaterialHistory.Reader(file,archive,track.getValue().getAsJsonArray());
                    } else if(GlintHistory.KIND.equals(track.getKey())) {
                        if(result.glintHistory!=null)throw new IOException("appearance archive has conflicting glint tracks");
                        result.reserve(16L*1024*1024);result.glintHistory=new GlintHistory.Reader(file,archive,track.getValue().getAsJsonArray());
                    } else if(ShaderClockHistory.KIND.equals(track.getKey())) {
                        if(result.shaderClockHistory!=null)throw new IOException("appearance archive has conflicting shader clock tracks");
                        result.reserve(16L*1024*1024);
                        result.shaderClockHistory=new ShaderClockHistory.Reader(file,archive,track.getValue().getAsJsonArray(),result.association,result.snapshot.session);
                    } else throw new IOException("unsupported appearance adapter track");
                }
            }
            return result;
        } catch(RuntimeException|StackOverflowError invalid) {throw new IOException("invalid recorded appearance",invalid);}
    }
    private static JsonObject historyState(AppearanceAssets.Archive archive,JsonElement value,Library library) throws IOException {
        JsonObject event=value.getAsJsonObject();whole(event,"timeMs",0,86_400_000);
        if(event.entrySet().size()!=2||!event.has("stateHash"))throw new IOException("invalid appearance history event descriptor");
        byte[] bytes=archive.read("asset",event.get("stateHash").getAsString());
        if(bytes.length>1024*1024||library.resident+bytes.length*4L>MAX_RESIDENT)throw new IOException("appearance history state exceeds preparation budget");
        library.resident+=bytes.length*4L;
        JsonObject state=new JsonParser().parse(new String(bytes,StandardCharsets.UTF_8)).getAsJsonObject();
        whole(state,"timeMs",0,86_400_000);
        if(state.get("timeMs").getAsLong()!=event.get("timeMs").getAsLong())throw new IOException("appearance history state time differs from its index");
        return state;
    }
    private ReplayAppearance(File file,AppearanceAssets.Archive archive,JsonObject state,Library library) throws IOException {
        source=file;manifestHash=archive.manifestHash;manifest=state;this.library=library;textures=library.textures;
        validateState(manifest);
        snapshot=GSON.fromJson(manifest,Snapshot.class);
        if(snapshot==null||snapshot.format!=1||snapshot.rig<1||snapshot.rig>2||snapshot.timeMs<0||snapshot.timeMs>86_400_000
                ||snapshot.session==null||!snapshot.session.matches("[A-Za-z0-9_-]{1,64}")
                ||snapshot.players==null||snapshot.players.length>128||snapshot.cosmetics==null||snapshot.cosmetics.length>500
                ||snapshot.emotes==null||snapshot.emotes.length>500)throw new IOException("invalid appearance snapshot header");
        heldTime=snapshot.timeMs;
        for(Actor actor:snapshot.players) {
            if(actor==null||!uuid(actor.id))throw new IOException("invalid appearance actor identity");
            actor.id=actor.id.toLowerCase(Locale.ROOT);
            if(actors.put(actor.id,actor)!=null)throw new IOException("duplicate appearance actor");
            if(actor.bedrockEquipmentSource!=null)actor.bedrockEquipment=equipment(archive,actor.bedrockEquipmentSource);
            else if(actor.bedrockEquipment!=null)actor.bedrockEquipment.validate();
        }
        for(JsonElement value:manifest.getAsJsonArray("players")) {
            JsonObject actor=value.getAsJsonObject();
            actors.get(actor.get("id").getAsString().toLowerCase(Locale.ROOT)).nativeSkinCaptured=actor.has("bedrockSkin");
            actors.get(actor.get("id").getAsString().toLowerCase(Locale.ROOT)).nativeCapeCaptured=actor.has("bedrockCape");
        }
        if(snapshot.localPlayer!=null) {
            if(!uuid(snapshot.localPlayer))throw new IOException("invalid recorded local player");
            snapshot.localPlayer=snapshot.localPlayer.toLowerCase(Locale.ROOT);
            if(!actors.containsKey(snapshot.localPlayer))throw new IOException("recorded local player is absent");
        }
    }
    private static void validateState(JsonObject manifest) throws IOException {
        whole(manifest,"format",1,1);whole(manifest,"rig",1,2);
        if(manifest.has("cosmeticTimeMs"))whole(manifest,"cosmeticTimeMs",0,1_000_000_000_000L);
        whole(manifest,"timeMs",0,86_400_000);flag(manifest,"reduceMotion");
        JsonElement players=manifest.get("players");
        if(players==null||!players.isJsonArray()||players.getAsJsonArray().size()>128)throw new IOException("invalid recorded appearance players");
        for(JsonElement value:players.getAsJsonArray()) {
            JsonObject actor=value.getAsJsonObject();
            if(!actor.has("look")||!actor.has("emote"))throw new IOException("recorded actor must declare look and emote state");
            if(actor.has("bedrockSkin")&&!actor.get("bedrockSkin").isJsonNull()) {
                JsonObject skin=actor.getAsJsonObject("bedrockSkin");flag(skin,"slim");
                JsonElement texture=skin.get("texture");
                if(skin.entrySet().size()!=2||texture==null||!texture.isJsonPrimitive()||!texture.getAsJsonPrimitive().isString()
                        ||!texture.getAsString().matches("[0-9a-f]{40}"))throw new IOException("invalid recorded native skin");
            }
            if(actor.has("bedrockCape")&&!actor.get("bedrockCape").isJsonNull()) {
                JsonObject cape=actor.getAsJsonObject("bedrockCape");flag(cape,"present");
                boolean present=cape.get("present").getAsBoolean();JsonElement texture=cape.get("texture");
                if(cape.entrySet().size()!=(present?2:1)||present&&(texture==null||!texture.isJsonPrimitive()
                        ||!texture.getAsJsonPrimitive().isString()||!texture.getAsString().matches("[0-9a-f]{40}"))
                        ||!present&&texture!=null)throw new IOException("invalid recorded native cape");
            }
            if(!actor.get("look").isJsonNull()) {
                JsonObject look=actor.getAsJsonObject("look");flag(look,"slim");flag(look,"arctic");whole(look,"capeFps",1,30);
                JsonElement fits=look.get("fits");
                if(fits==null||!fits.isJsonObject())throw new IOException("invalid recorded appearance fits");
                for(Map.Entry<String,JsonElement> entry:fits.getAsJsonObject().entrySet()) {
                    JsonObject fit=entry.getValue().getAsJsonObject();whole(fit,"up",-6,6);whole(fit,"forward",-4,4);flag(fit,"mirror");
                }
            }
            if(!actor.get("emote").isJsonNull()) {
                JsonObject emote=actor.getAsJsonObject("emote");flag(emote,"held");
                if(emote.has("elapsedMillis")) {
                    JsonElement elapsed=emote.get("elapsedMillis");
                    if(emote.get("held").getAsBoolean()||!elapsed.isJsonPrimitive()||!elapsed.getAsJsonPrimitive().isNumber()
                            ||!Double.isFinite(elapsed.getAsDouble())||elapsed.getAsDouble()<0||elapsed.getAsDouble()>86_400_000)
                        throw new IOException("invalid recorded emote millisecond phase");
                }
            }
            if(actor.has("bedrockEquipment")&&!actor.get("bedrockEquipment").isJsonNull()) {
                validateEquipment(actor.getAsJsonObject("bedrockEquipment"));
            }
            JsonElement equipmentSource=actor.get("bedrockEquipmentSource");
            if(equipmentSource!=null&&!equipmentSource.isJsonNull()) {
                if(actor.has("bedrockEquipment")&&!actor.get("bedrockEquipment").isJsonNull())throw new IOException("conflicting recorded equipment sources");
                if(!equipmentSource.isJsonObject())throw new IOException("invalid recorded equipment source");
                JsonObject ref=equipmentSource.getAsJsonObject();whole(ref,"schema",1,1);
                JsonElement hash=ref.get("hash");
                if(ref.entrySet().size()!=2||hash==null||!hash.isJsonPrimitive()||!hash.getAsJsonPrimitive().isString()
                        ||!hash.getAsString().matches("[0-9a-f]{40}"))throw new IOException("invalid recorded equipment source fields");
            }
        }
    }
    static void validateEquipment(JsonObject equipment) throws IOException {
        if(equipment.entrySet().size()!=2||!equipment.has("armor")||!equipment.has("hands"))throw new IOException("invalid recorded equipment groups");
        for(String group:new String[]{"armor","hands"}) {
            JsonElement slots=equipment.get(group);if(slots.isJsonNull())continue;
            if(!slots.isJsonArray()||slots.getAsJsonArray().size()!=("armor".equals(group)?4:2))throw new IOException("invalid recorded equipment slot count");
            for(JsonElement entry:slots.getAsJsonArray()) {
                JsonObject slot=entry.getAsJsonObject();flag(slot,"empty");
                if(slot.entrySet().size()!=3+(slot.has("iconKey")?1:0)+(slot.has("iconFrame")?1:0)
                        +(slot.has("enchanted")?1:0)+(slot.has("forcedGlint")?1:0)+(slot.has("nativeGlint")?1:0)
                        +(slot.has("enchantmentListSize")?1:0)+(slot.has("enchantmentListElementType")?1:0)
                        +(slot.has("enchantmentEntries")?1:0)
                        +(slot.has("javaEnchantments")?1:0)
                        +(slot.has("javaFoilInputs")?1:0)+(slot.has("javaFoil")?1:0)
                        +(slot.has("javaCapeLayers")?1:0)
                        ||!slot.has("item")||!slot.has("count"))throw new IOException("invalid recorded equipment slot");
                if(!slot.get("count").isJsonNull())whole(slot,"count",0,255);
                JsonElement item=slot.get("item");
                if(!item.isJsonNull()&&(!item.isJsonPrimitive()||!item.getAsJsonPrimitive().isString()))throw new IOException("invalid recorded equipment item ID");
                JsonElement icon=slot.get("iconKey");
                if(icon!=null&&!icon.isJsonNull()&&(!icon.isJsonPrimitive()||!icon.getAsJsonPrimitive().isString()))throw new IOException("invalid recorded item icon key");
                JsonElement frame=slot.get("iconFrame");if(frame!=null&&!frame.isJsonNull())whole(slot,"iconFrame",0,65535);
                if(slot.has("enchantmentListSize")&&!slot.get("enchantmentListSize").isJsonNull())whole(slot,"enchantmentListSize",0,256);
                if(slot.has("enchantmentListElementType")&&!slot.get("enchantmentListElementType").isJsonNull())whole(slot,"enchantmentListElementType",0,11);
                JsonElement rawEntries=slot.get("enchantmentEntries");
                JavaEnchantmentComponents.validateJson(slot.get("javaEnchantments"));
                JavaItemFoil.validateJson(slot.get("javaFoilInputs"));
                JavaCapeLayer.validateJson(slot.get("javaCapeLayers"));
                if(rawEntries!=null&&!rawEntries.isJsonNull()) {
                    if(!rawEntries.isJsonArray()||rawEntries.getAsJsonArray().size()>64)throw new IOException("invalid recorded enchantment entry count");
                    for(JsonElement rawEntry:rawEntries.getAsJsonArray()) {
                        if(!rawEntry.isJsonObject())throw new IOException("invalid recorded enchantment entry");
                        JsonObject enchantment=rawEntry.getAsJsonObject();
                        if(enchantment.entrySet().size()!=2||!enchantment.has("id")||!enchantment.has("level"))throw new IOException("invalid recorded enchantment fields");
                        for(String key:new String[]{"id","level"})if(!enchantment.get(key).isJsonNull())whole(enchantment,key,Short.MIN_VALUE,Short.MAX_VALUE);
                    }
                }
                for(String key:new String[]{"enchanted","forcedGlint","nativeGlint","javaFoil"})
                    if(slot.has(key)&&!slot.get(key).isJsonNull())flag(slot,key);
            }
        }
    }
    private WorldFrame.Equipment equipment(AppearanceAssets.Archive archive,EquipmentSource source) throws IOException {
        WorldFrame.Equipment known=library.equipment.get(source.hash);if(known!=null)return known;
        byte[] bytes=archive.read("asset",source.hash,NativeEquipmentCapture.MAX_BYTES);
        reserve(bytes.length*8L+512);
        JsonObject json=new JsonParser().parse(new String(bytes,StandardCharsets.UTF_8)).getAsJsonObject();
        validateEquipment(json);
        WorldFrame.Equipment parsed=GSON.fromJson(json,WorldFrame.Equipment.class);parsed.validate();
        // Private archive-owned data is read-only after validation. Sharing it
        // avoids re-preparing the same equipment across unrelated look edits.
        library.equipment.put(source.hash,parsed);return parsed;
    }
    private static void flag(JsonObject object,String key) throws IOException {
        JsonElement value=object.get(key);
        if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isBoolean())throw new IOException("invalid appearance flag: "+key);
    }
    private static void whole(JsonObject object,String key,long min,long max) throws IOException {
        JsonElement value=object.get(key);
        if(value==null||!value.isJsonPrimitive()||!value.getAsJsonPrimitive().isNumber())throw new IOException("invalid appearance integer: "+key);
        double number=value.getAsDouble();long integer=value.getAsLong();
        if(!Double.isFinite(number)||number!=integer||integer<min||integer>max)throw new IOException("appearance integer is out of range: "+key);
    }
    private void prepare(AppearanceAssets.Archive archive) throws IOException {
        for(Model model:snapshot.cosmetics) {
            if(model==null||!id(model.id)||!id(model.slot)||models.containsKey(model.id))throw new IOException("invalid or duplicate recorded cosmetic");
            String key=GSON.toJson(model);AvatarRenderer.Cosmetic baked=library.models.get(key);
            if(baked!=null) {models.put(model.id,baked);continue;}
            if(model.mesh!=null) {
                if(model.model!=null||model.texture!=null||model.glow!=null||model.animation!=null)throw new IOException("ambiguous recorded cosmetic format");
                byte[] bytes=archive.read("asset",model.mesh);MeshModel mesh=MeshModel.parse(bytes);long cost=bytes.length;
                for(MeshModel.Primitive p:mesh.primitives)cost+=4L*(p.positions.length+p.normals.length+p.uvs.length+p.colors.length+p.indices.length);
                reserve(cost);
                for(byte[] png:mesh.images)imageSize(png,0,true);
                baked=AvatarRenderer.mesh(model.id,mesh);
            } else {
                Geometry geometry=Geometry.parse(json(archive,model.model));long cost=0;
                for(Geometry.Bone bone:geometry.bones)cost+=256L+bone.cubes.size()*1024L;
                reserve(cost);
                Animation idle=model.animation==null?null:animation(archive,model.animation);
                baked=AvatarRenderer.cuboid(model.id,geometry,idle,modelImage(archive,model.texture),model.glow==null?null:modelImage(archive,model.glow));
            }
            models.put(model.id,baked);library.models.put(key,baked);
        }
        for(Move move:snapshot.emotes) {
            if(move==null||!id(move.id)||moves.containsKey(move.id))throw new IOException("invalid or duplicate recorded emote");
            moves.put(move.id,animation(archive,move.animation));
        }
        for(Actor actor:snapshot.players) {
            if(actor.bedrockSkin!=null&&!textures.containsKey("look:"+actor.bedrockSkin.texture))
                textures.put("look:"+actor.bedrockSkin.texture,appearanceImage(archive.read("texture",actor.bedrockSkin.texture),1));
            if(actor.bedrockCape!=null&&actor.bedrockCape.present) {
                String hash=actor.bedrockCape.texture,key=nativeCapeKey(hash);
                if(!textures.containsKey(key))textures.put(key,appearanceImage(archive.read("texture",hash),3));
                if(!library.nativeCapeIdentities.containsKey(hash)) {
                    reserve(256);library.nativeCapeIdentities.put(hash,NativeImageIdentity.capeKey(textures.get(key)));
                }
            }
            LookAt look=actor.look;
            if(look!=null) {
                if(look.cosmetics==null||look.cosmetics.length>8||look.fits==null)throw new IOException("invalid recorded worn cosmetics");
                Set<String> worn=new HashSet<String>();
                for(String item:look.cosmetics)if(!models.containsKey(item)||!worn.add(item))throw new IOException("recorded cosmetic definition is missing or repeated");
                for(Map.Entry<String,FitAt> entry:look.fits.entrySet()) {
                    FitAt fit=entry.getValue();
                    if(!worn.contains(entry.getKey())||fit==null||fit.up< -6||fit.up>6||fit.forward< -4||fit.forward>4)throw new IOException("invalid recorded cosmetic fit");
                }
                if(look.skin!=null) {
                    if(!textures.containsKey("look:"+look.skin))textures.put("look:"+look.skin,appearanceImage(archive.read("texture",look.skin),1));
                    library.javaTextureIds.put("look:"+look.skin,JavaTextureId.look(look.skin));
                }
                if(look.cape!=null) {
                    if(look.capeFps<1||look.capeFps>30)throw new IOException("invalid recorded cape frame rate");
                    Integer old=rates.put(look.cape,look.capeFps);
                    if(old!=null&&old!=look.capeFps)throw new IOException("conflicting recorded cape frame rates");
                    if(!library.frames.containsKey(look.cape)) {
                        BufferedImage image=appearanceImage(archive.read("texture",look.cape),2);
                        int count=Looks.capeFrames(image.getWidth(),image.getHeight()),height=image.getHeight()/count;library.frames.put(look.cape,count);
                        for(int f=0;f<count;f++) {
                            String key="look:"+look.cape+"/"+f;
                            textures.put(key,image.getSubimage(0,f*height,image.getWidth(),height));
                            // The replay's /0 alias also represents a static
                            // cape. Fabric registers that one at the bare hash.
                            library.javaTextureIds.put(key,JavaTextureId.look(look.cape+(count==1?"":"/"+f)));
                        }
                    }
                }
            }
            if(actor.emote!=null&&(!moves.containsKey(actor.emote.id)||!Float.isFinite(actor.emote.seconds)
                    ||actor.emote.seconds<0||actor.emote.seconds>86_400))throw new IOException("invalid recorded emote phase or definition");
            if(actor.emote!=null&&actor.emote.elapsedMillis!=null
                    &&Float.floatToIntBits((float)(actor.emote.elapsedMillis/1000d))!=Float.floatToIntBits(actor.emote.seconds))
                throw new IOException("recorded emote phases disagree");
        }
    }
    private Animation animation(AppearanceAssets.Archive archive,String hash) throws IOException {
        Animation value=library.animations.get(hash);
        if(value==null) {
            byte[] bytes=archive.read("asset",hash);reserve(bytes.length*4L);
            value=Animation.parse(new JsonParser().parse(new String(bytes,StandardCharsets.UTF_8)));library.animations.put(hash,value);
        }
        return value;
    }
    private static JsonElement json(AppearanceAssets.Archive archive,String hash) throws IOException {
        return new JsonParser().parse(new String(archive.read("asset",hash),StandardCharsets.UTF_8));
    }
    private BufferedImage modelImage(AppearanceAssets.Archive archive,String hash) throws IOException {
        BufferedImage image=library.modelTextures.get(hash);
        if(image==null) {image=appearanceImage(archive.read("texture",hash),0);library.modelTextures.put(hash,image);}
        return image;
    }
    // 0: model image; 1: prepared skin; 2: Arctic animated cape;
    // 3: one raw native cape, with no generated wing pixels.
    private int[] imageSize(byte[] bytes,int mode,boolean count) throws IOException {
        if(mode==0&&bytes.length>1024*1024)throw new IOException("cosmetic PNG exceeds 1 MiB");
        try(ImageInputStream input=ImageIO.createImageInputStream(new ByteArrayInputStream(bytes))) {
            if(input==null)throw new IOException("invalid appearance PNG stream");
            Iterator<ImageReader> readers=ImageIO.getImageReaders(input);
            if(!readers.hasNext())throw new IOException("invalid appearance PNG");
            ImageReader reader=readers.next();
            try {
                if(!"png".equalsIgnoreCase(reader.getFormatName()))throw new IOException("appearance image is not PNG");
                reader.setInput(input,true,true);int w=reader.getWidth(0),h=reader.getHeight(0);
                if(w<1||h<1||w>1024||mode==0&&h>1024
                        ||mode==1&&(w%64!=0||h!=w&&h*2!=w)||mode==2&&Looks.capeFrames(w,h)<1
                        ||mode==3&&(w<64||w%64!=0||h*2!=w))
                    throw new IOException("appearance image dimensions do not match its role");
                if(count)reserve(4L*w*(mode==1?w:h));return new int[]{w,h};
            } finally {reader.dispose();}
        }
    }
    private BufferedImage appearanceImage(byte[] bytes,int mode) throws IOException {
        imageSize(bytes,mode,true);BufferedImage image=ImageIO.read(new ByteArrayInputStream(bytes));
        if(image==null)throw new IOException("invalid appearance image pixels");
        if(mode==1)image=SkinFormat.modern(image);
        else if(mode==2) {BufferedImage prepared=CapeFormat.withElytra(image);if(prepared!=null)image=prepared;}
        if(image==null)throw new IOException("appearance image cannot be prepared");return image;
    }
    private void reserve(long bytes) throws IOException {
        if(bytes<0||library.resident+bytes>MAX_RESIDENT)throw new IOException("recorded appearance exceeds the 80 MiB resident preparation budget");library.resident+=bytes;
    }
    private static boolean id(String id) {return id!=null&&id.matches("[a-z0-9_-]{1,32}");}
    private static boolean uuid(String value) {return value!=null&&value.length()==36&&UUID.fromString(value).toString().equalsIgnoreCase(value);}

    void validateViewer(WorldFrame world) throws IOException {
        if(timeline!=null) {for(ReplayAppearance event:timeline.values())event.validateViewerAt(world);}
        else validateViewerAt(world);
    }
    private void validateViewerAt(WorldFrame world) throws IOException {
        if(snapshot.localPlayer!=null) {
            if(world.localPlayer==null)throw new IOException("recorded appearance needs an initialized local viewer");
            if(!world.localPlayer.equals(snapshot.localPlayer)&&actors.containsKey(world.localPlayer))throw new IOException("local appearance remapping would shadow a recorded remote player");
        }
    }
    void setTime(long time) throws IOException {
        if(time<snapshot.timeMs||time>endTime)throw new IOException("appearance time is outside the recorded range");heldTime=time;
    }
    long time(WorldFrame world) {
        Long recorded=world.replayCosmeticTimeMillis();long time=recorded==null?heldTime:recorded;
        if(time<snapshot.timeMs||time>endTime||recorded!=null&&world.cosmeticTimeExactMillis()>endTime)
            throw new IllegalArgumentException("appearance sample is outside the recorded range");return time;
    }
    private double exactTime(WorldFrame world) {
        long whole=time(world);return world.replayCosmeticSample()==null?whole:world.cosmeticTimeExactMillis();
    }
    private ReplayAppearance selected(long time) {return timeline==null?this:timeline.floorEntry(time).getValue();}
    private Actor actor(UUID id,WorldFrame world) {
        String key=id.toString();
        if(snapshot.localPlayer!=null&&key.equals(world.localPlayer))key=snapshot.localPlayer;
        else if(snapshot.localPlayer!=null&&key.equals(snapshot.localPlayer)&&!key.equals(world.localPlayer))return null;
        return actors.get(key);
    }
    private double elapsedMillis(Actor actor,double time) {return actor.emote.elapsedMillis+(time-snapshot.timeMs);}
    private float seconds(Actor actor,double time) {
        if(actor.emote.held)return actor.emote.seconds;
        return actor.emote.elapsedMillis==null?(float)(actor.emote.seconds+(time-snapshot.timeMs)/1000d)
            :(float)(elapsedMillis(actor,time)/1000d);
    }
    EmotePrograms.Seed emoteSeed(UUID id,WorldFrame world) {
        long time=time(world);ReplayAppearance event=selected(time);Actor actor=event.actor(id,world);
        if(actor==null||actor.emote==null)return null;
        Animation animation=event.moves.get(actor.emote.id);
        if(animation==null||animation.finished(event.seconds(actor,exactTime(world))))return null;
        Long next=timeline==null?null:timeline.higherKey(event.snapshot.timeMs);
        long end=next==null?endTime:Math.min(endTime,next-1);
        BodyHistory.Curve reference=bodyCurve(id,world);
        if(bodyHistory!=null&&reference==null)return null;
        long start=event.snapshot.timeMs;
        if(reference!=null) {start=Math.max(start,reference.firstWholeMillis());end=Math.min(end,reference.lastWholeMillis());}
        if(start>end||time<start||time>end)return null;
        return new EmotePrograms.Seed(id.toString(),actor,animation,event.snapshot.timeMs,actor.emote.seconds,
            actor.emote.held,true,start,end,reference,actor.emote.elapsedMillis);
    }
    private long cosmeticTime(long time) {return (snapshot.cosmeticTimeMs==null?snapshot.timeMs:snapshot.cosmeticTimeMs)+(time-snapshot.timeMs);}
    private double cosmeticTime(double time) {return (snapshot.cosmeticTimeMs==null?snapshot.timeMs:snapshot.cosmeticTimeMs)+(time-snapshot.timeMs);}
    boolean hasCapeHistory() {return capeHistory!=null;}
    boolean hasJavaWingHistory(){return capeHistory!=null&&capeHistory.hasWingPhysics();}
    boolean hasBodyHistory() {return bodyHistory!=null;}
    boolean hasBodyColors() {return bodyHistory!=null&&bodyHistory.hasColors();}
    boolean hasBodyPackedLight() {return bodyHistory!=null&&bodyHistory.hasPackedLight();}
    boolean hasBodySkinMaterials() {return bodyHistory!=null&&bodyHistory.hasSkinMaterials();}
    boolean hasBodySkinVisibility() {return bodyHistory!=null&&bodyHistory.hasSkinVisibility();}
    boolean hasBodyLayerVisibility() {return bodyHistory!=null&&bodyHistory.hasLayerVisibility();}
    JavaEntityShader entityShader(WorldFrame world) {
        if(entityMaterialHistory==null){entityMaterialHistoryError="appearance archive has no recorded entity material track";return null;}
        try {
            double time=exactTime(world);JavaEntityShader shader=entityMaterialHistory.sample(time,world);
            entityMaterialHistoryError=shader==null?"no matching recorded entity material/camera context at "+time+" ms":null; // i18n-ignore
            return shader;
        }catch(IOException|RuntimeException failure){entityMaterialHistoryError=failure.toString();return null;}
    }
    boolean hasLayerColors() {return layerColorHistory!=null;}
    boolean hasLayerDithering(){return layerColorHistory!=null&&layerColorHistory.hasDithering();}
    boolean hasLayerShaderInputs(){return layerColorHistory!=null&&layerColorHistory.hasShaderInputs();}
    boolean hasLayerMaterialContext(){return layerColorHistory!=null&&layerColorHistory.hasMaterialContext();}
    boolean hasLayerMaterialPaths(){return layerColorHistory!=null&&layerColorHistory.hasMaterialPaths();}
    boolean hasLayerMaterialVariation(){return layerColorHistory!=null&&layerColorHistory.hasMaterialVariation();}
    boolean hasNativeCapePaletteTrack(){return attachmentHistory!=null&&attachmentHistory.hasNativeCapes();}
    boolean hasNativeWingPaletteTrack(){return attachmentHistory!=null&&attachmentHistory.hasNativeWings();}
    LayerColorSample layerColors(UUID id,WorldFrame world) {
        if(layerColorHistory==null)return null;
        try {
            double time=exactTime(world);Actor actor=selected((long)time).actor(id,world);if(actor==null)return null;
            LayerColorSample sample=layerColorHistory.sample(UUID.fromString(actor.id),time);
            layerColorHistoryError=sample==null?"no contiguous recorded layer colors for "+actor.id+" at "+time+" ms":null;return sample;
        }catch(IOException|RuntimeException failure){layerColorHistoryError=failure.toString();return null;}
    }
    boolean capeColorReady(UUID id,WorldFrame world) {
        Actor actor=selected(time(world)).actor(id,world);
        if(actor==null)return false;
        if(layerColorHistory!=null) {
            LayerColorSample sample=layerColors(id,world);if(sample==null)return false;
            int state=sample.state(LayerColorSample.CAPE);
            if(state==0||actor.bedrockCape!=null&&(state==2)!=actor.bedrockCape.present)return false;
            return true;
        }
        // An explicitly absent native cape has no native tint to reproduce.
        return actor.bedrockCape!=null&&!actor.bedrockCape.present;
    }
    boolean wingColorReady(UUID id,WorldFrame world) {
        LayerColorSample sample=layerColors(id,world);return sample!=null&&sample.state(LayerColorSample.WING)==2;
    }
    boolean hasModelRoots(){return bodyHistory!=null&&bodyHistory.hasRoots();}
    String manifestHash(){return manifestHash;}
    private BodyHistory.Curve bodyCurve(UUID id,WorldFrame world) {
        if(bodyHistory==null) {bodyHistoryError="appearance archive has no recorded body samples";return null;}
        double time=exactTime(world);Actor actor=selected((long)time).actor(id,world);if(actor==null)return null;
        try {
            BodyHistory.Curve curve=bodyHistory.curve(UUID.fromString(actor.id),time);
            bodyHistoryError=curve==null?"no contiguous recorded body samples for "+actor.id+" at "+time+" ms":null;
            return curve;
        } catch(IOException|RuntimeException failure) {bodyHistoryError=failure.toString();return null;}
    }
    BodySample bodyPose(UUID id,WorldFrame world) {
        try {BodyHistory.Curve curve=bodyCurve(id,world);return curve==null?null:curve.sample(exactTime(world));}
        catch(RuntimeException failure) {bodyHistoryError=failure.toString();return null;}
    }
    float[] modelRoot(UUID id,WorldFrame world) {
        try {
            BodySample sample=bodyPose(id,world);if(sample==null||sample.root==null)return null;
            if(!Boolean.TRUE.equals(world.cameraRelative))throw new IllegalArgumentException("native model-root playback needs declared camera-relative coordinates");
            return sample.root.relative(world.camera);
        }
        catch(RuntimeException failure){bodyHistoryError=failure.toString();return null;}
    }
    AttachmentSample attachmentPose(UUID id,WorldFrame world) {
        if(attachmentHistory==null){attachmentHistoryError="appearance archive has no recorded attachment samples";return null;}
        try {
            double time=exactTime(world);Actor actor=selected((long)time).actor(id,world);if(actor==null)return null;
            AttachmentSample sample=attachmentHistory.sample(UUID.fromString(actor.id),time);
            attachmentHistoryError=sample==null?"no contiguous recorded attachment samples for "+actor.id+" at "+time+" ms":null;return sample;
        }catch(IOException|RuntimeException failure){attachmentHistoryError=failure.toString();return null;}
    }
    WingSample wingPose(UUID id,WorldFrame world) {
        if(wingHistory==null) {wingHistoryError="appearance archive has no recorded wing samples";return null;}
        double time=exactTime(world);Actor actor=selected((long)time).actor(id,world);if(actor==null)return null;
        try {
            WingSample sample=wingHistory.sample(UUID.fromString(actor.id),time);
            wingHistoryError=sample==null?"no contiguous recorded wing samples for "+actor.id+" at "+time+" ms":null;
            return sample;
        } catch(IOException|RuntimeException failure) {wingHistoryError=failure.toString();return null;}
    }
    GlintSample wingGlint(UUID id,WorldFrame world) {
        if(glintHistory==null){glintHistoryError="appearance archive has no recorded glint inputs";return null;}
        double time=exactTime(world);Actor actor=selected((long)time).actor(id,world);
        if(actor==null){glintHistoryError="no recorded glint actor at "+time+" ms";return null;}
        try {
            GlintSample sample=glintHistory.sample(UUID.fromString(actor.id),time,world);
            glintHistoryError=sample==null?"no exact/stable camera-compatible glint observation for "+actor.id+" at "+time+" ms":null; // i18n-ignore
            return sample;
        }catch(IOException|RuntimeException failure){glintHistoryError=failure.toString();return null;}
    }
    CapePhysics.Motion capeMotion(UUID id,WorldFrame world) {
        if(capeHistory==null)return null;
        double time=exactTime(world);Actor actor=selected((long)time).actor(id,world);if(actor==null)return null;
        try {
            CapePhysics.Motion motion=capeHistory.sample(UUID.fromString(actor.id),time);
            if(motion==null)capeHistoryError="no recorded cape tick for "+actor.id+" at "+time+" ms";
            return motion;
        } catch(IOException|RuntimeException failure) {capeHistoryError=failure.toString();return null;}
    }
    JavaWingPhysics.Motion javaWingMotion(UUID id,WorldFrame world) {
        if(!hasJavaWingHistory())return null;
        double time=exactTime(world);Actor actor=selected((long)time).actor(id,world);if(actor==null)return null;
        try {
            JavaWingPhysics.Motion motion=capeHistory.wing(UUID.fromString(actor.id),time);
            wingHistoryError=motion==null?"no recorded Java wing tick for "+actor.id+" at "+time+" ms":null; // i18n-ignore
            return motion;
        }catch(IOException|RuntimeException failure){wingHistoryError=failure.toString();return null;}
    }
    AvatarRenderer.Avatar avatar(UUID id,WorldFrame world) {
        long time=time(world);return selected(time).avatarAt(id,world,exactTime(world));
    }
    private AvatarRenderer.Avatar avatarAt(UUID id,WorldFrame world,double time) {
        AvatarRenderer.Avatar avatar=new AvatarRenderer.Avatar();
        avatar.seconds=(float)((cosmeticTime(time)%3_600_000)/1000d);avatar.reduceMotion=snapshot.reduceMotion;Actor actor=actor(id,world);
        if(actor==null)return avatar;
        avatar.javaCapeLayer=JavaCapeLayer.select(actor.bedrockEquipment);
        LookAt look=actor.look;
        if(look!=null) {
            avatar.slim=look.slim;
            String skinKey=look.skin==null?null:"look:"+look.skin;
            String capeKey=look.cape==null?null:capeFrameAt(look.cape,(long)time);
            avatar.skinTexture(textures.get(skinKey),textureId(skinKey));
            avatar.capeTexture(textures.get(capeKey),textureId(capeKey));
            for(String item:look.cosmetics) {
                FitAt fit=look.fits.get(item);
                avatar.worn.add(new AvatarRenderer.Worn(models.get(item),fit==null?Fit.NONE:Fit.of(fit.up,fit.forward,fit.mirror)));
            }
        }
        if(avatar.skin==null&&actor.bedrockSkin!=null) {
            avatar.skinTexture(textures.get("look:"+actor.bedrockSkin.texture),null);avatar.slim=actor.bedrockSkin.slim;
        }
        if(avatar.cape==null&&actor.bedrockCape!=null&&actor.bedrockCape.present)
            avatar.capeTexture(textures.get(nativeCapeKey(actor.bedrockCape.texture)),null);
        if(actor.emote!=null) {
            float at=seconds(actor,time);Animation move=moves.get(actor.emote.id);
            if(!move.finished(at)) {avatar.emote=move;avatar.emoteSeconds=at;}
        }
        return avatar;
    }
    private static String skin(Actor actor) {
        if(actor==null)return null;
        if(actor.look!=null&&actor.look.skin!=null)return actor.look.skin;
        return actor.bedrockSkin==null?null:actor.bedrockSkin.texture;
    }
    String skin(UUID id,WorldFrame world) {return skin(selected(time(world)).actor(id,world));}
    boolean nativeSkinCaptured(UUID id,WorldFrame world) {
        Actor actor=selected(time(world)).actor(id,world);return actor!=null&&actor.nativeSkinCaptured;
    }
    boolean skinReady(UUID id,WorldFrame world) {
        Actor actor=selected(time(world)).actor(id,world);
        return actor!=null&&(!actor.nativeSkinCaptured||skin(actor)!=null);
    }
    private static String nativeCapeKey(String hash) {return "look:"+hash+"/native";}
    String textureId(String key){return library.javaTextureIds.get(key);}
    private static String cape(Actor actor) {
        if(actor==null)return null;
        if(actor.look!=null&&actor.look.cape!=null)return actor.look.cape;
        return actor.bedrockCape!=null&&actor.bedrockCape.present?actor.bedrockCape.texture:null;
    }
    String cape(UUID id,WorldFrame world) {return cape(selected(time(world)).actor(id,world));}
    String capeTexture(UUID id,WorldFrame world) {
        long time=time(world);ReplayAppearance event=selected(time);Actor actor=event.actor(id,world);
        String hash=cape(actor);if(hash==null)return null;
        return actor.look!=null&&actor.look.cape!=null?event.capeFrameAt(hash,time):nativeCapeKey(hash);
    }
    boolean usesNativeCape(UUID id,WorldFrame world) {
        Actor actor=selected(time(world)).actor(id,world);
        return actor!=null&&(actor.look==null||actor.look.cape==null)&&actor.bedrockCape!=null&&actor.bedrockCape.present;
    }
    String nativeCapeIdentity(UUID id,WorldFrame world) {
        Actor actor=selected(time(world)).actor(id,world);
        return actor==null||actor.bedrockCape==null||!actor.bedrockCape.present?null:library.nativeCapeIdentities.get(actor.bedrockCape.texture);
    }
    // A native cape PNG alone does not establish native elytra art or visibility.
    String wingCape(UUID id,WorldFrame world) {
        Actor actor=selected(time(world)).actor(id,world);return actor==null||actor.look==null?null:actor.look.cape;
    }
    boolean capeReady(UUID id,WorldFrame world) {
        Actor actor=selected(time(world)).actor(id,world);
        return actor!=null&&(!actor.nativeCapeCaptured||actor.look!=null&&actor.look.cape!=null||actor.bedrockCape!=null);
    }
    boolean nativeCapeAbsent(UUID id,WorldFrame world) {
        Actor actor=selected(time(world)).actor(id,world);
        return actor!=null&&actor.nativeCapeCaptured&&actor.bedrockCape!=null&&!actor.bedrockCape.present&&cape(actor)==null;
    }
    boolean equipmentMatches(UUID id,WorldFrame world,WorldFrame.Equipment actual) {
        Actor actor=selected(time(world)).actor(id,world);
        boolean matches=actor==null||actor.bedrockEquipment==null||actor.bedrockEquipment.matches(actual);
        equipmentHistoryError=matches?null:"recorded equipment does not match actor "+id;
        return matches;
    }
    String capeFrame(String hash,long time) {
        return selected(time).capeFrameAt(hash,time);
    }
    private String capeFrameAt(String hash,long time) {
        Integer count=library.frames.get(hash),rate=rates.get(hash);if(count==null||rate==null)return null;
        int frame=snapshot.reduceMotion?0:(int)Math.floorMod(cosmeticTime(time)/(1000L/rate),count.longValue());
        return "look:"+hash+"/"+frame;
    }
    void save(AppearanceAssets store,WorldFrame world,File output) throws IOException {
        long time=time(world);selected(time).saveAt(store,world,output,time);
    }
    private void saveAt(AppearanceAssets store,WorldFrame world,File output,long time) throws IOException {
        if(cosmeticTime(time)>1_000_000_000_000L)throw new IOException("current cosmetic phase exceeds the appearance format limit");
        JsonObject state=new JsonParser().parse(manifest.toString()).getAsJsonObject();
        state.addProperty("session",world.session);state.addProperty("timeMs",time);state.addProperty("localPlayer",snapshot.localPlayer==null?null:world.localPlayer);
        state.addProperty("cosmeticTimeMs",cosmeticTime(time));
        for(JsonElement element:state.getAsJsonArray("players")) {
            JsonObject actor=element.getAsJsonObject();String id=actor.get("id").getAsString().toLowerCase(Locale.ROOT);Actor original=actors.get(id);
            if(id.equals(snapshot.localPlayer))actor.addProperty("id",world.localPlayer);
            if(original.emote!=null) {
                float phase=seconds(original,time);
                if(!Float.isFinite(phase)||phase>86_400)throw new IOException("current emote phase exceeds the appearance format limit");
                actor.getAsJsonObject("emote").addProperty("seconds",phase);
                if(original.emote.elapsedMillis!=null) {
                    double elapsed=elapsedMillis(original,time);
                    if(!Double.isFinite(elapsed)||elapsed<0||elapsed>86_400_000)throw new IOException("current emote millisecond phase exceeds the appearance format limit");
                    actor.getAsJsonObject("emote").addProperty("elapsedMillis",elapsed);
                }
            }
        }
        try(AppearanceAssets.Archive archive=new AppearanceAssets.Archive(source)) {
            if(!manifestHash.equals(archive.manifestHash))throw new IOException("source appearance manifest changed after loading");
            store.archive(output,state,archive.references(),archive::read);
        }
    }
    List<NativeShaderClock> shaderClockObservations(double presentationTimeMs) throws IOException {
        return shaderClockHistory==null?Collections.<NativeShaderClock>emptyList():shaderClockHistory.observationsAt(presentationTimeMs);
    }
    JsonObject status(WorldFrame world) {
        JsonObject state=new JsonObject();state.addProperty("manifestHash",manifestHash);state.addProperty("sourceTimeMs",snapshot.timeMs);
        long time=time(world);ReplayAppearance selected=selected(time);
        state.addProperty("timeMs",time);state.addProperty("endTimeMs",endTime);state.addProperty("snapshotTimeMs",selected.snapshot.timeMs);
        state.addProperty("sampleTimeMs",exactTime(world));
        state.addProperty("cosmeticTimeMs",selected.cosmeticTime(time));
        state.addProperty("capePhysicsRecorded",capeHistory!=null);state.addProperty("lastCapePhysicsError",capeHistoryError);
        state.addProperty("javaWingPhysicsTrack",hasJavaWingHistory());state.addProperty("javaWingPhysicsVerified",false);
        state.addProperty("wingSamplesRecorded",wingHistory!=null);state.addProperty("lastWingSampleError",wingHistoryError);
        state.addProperty("wingGlintTrackAvailable",glintHistory!=null);state.addProperty("lastWingGlintSampleError",glintHistoryError);
        state.addProperty("shaderClockTrackAvailable",shaderClockHistory!=null);
        if(shaderClockHistory!=null) {
            state.addProperty("shaderClockPresentationStartMs",shaderClockHistory.firstTime());
            state.addProperty("shaderClockPresentationEndMs",shaderClockHistory.lastTime());
        }
        state.addProperty("recordedShaderPhaseApplied",false);
        state.addProperty("wingGlintPlaybackVerified",false);
        state.addProperty("bodySamplesRecorded",bodyHistory!=null);state.addProperty("lastBodySampleError",bodyHistoryError);
        state.addProperty("bodyColorsRecorded",bodyHistory!=null&&bodyHistory.hasColors());state.addProperty("bodyColorInterpolationVerified",false);
        state.addProperty("bodyPackedLightTrackAvailable",hasBodyPackedLight());state.addProperty("bodyPackedLightPlaybackVerified",false);
        state.addProperty("bodySkinMaterialTrackAvailable",hasBodySkinMaterials());state.addProperty("bodySkinMaterialPlaybackVerified",false);
        state.addProperty("bodySkinVisibilityTrackAvailable",hasBodySkinVisibility());state.addProperty("bodySkinVisibilityPlaybackVerified",false);
        state.addProperty("bodyLayerVisibilityTrackAvailable",hasBodyLayerVisibility());state.addProperty("bodyLayerVisibilityPlaybackVerified",false);
        state.addProperty("entityMaterialTrackAvailable",entityMaterialHistory!=null);state.addProperty("lastEntityMaterialError",entityMaterialHistoryError);
        state.addProperty("entityMaterialPlaybackVerified",false);
        state.addProperty("layerColorsRecorded",layerColorHistory!=null);state.addProperty("lastLayerColorError",layerColorHistoryError);
        state.addProperty("layerColorInterpolationVerified",false);
        state.addProperty("bodyInterpolationVerified",false);
        state.addProperty("modelRootsRecorded",hasModelRoots());state.addProperty("modelRootInterpolationVerified",false);
        state.addProperty("attachmentSamplesRecorded",attachmentHistory!=null);state.addProperty("lastAttachmentSampleError",attachmentHistoryError);
        state.addProperty("nativeCapePaletteTrackAvailable",hasNativeCapePaletteTrack());state.addProperty("nativeCapePaletteRenderingVerified",false);
        state.addProperty("nativeWingPaletteTrackAvailable",hasNativeWingPaletteTrack());state.addProperty("nativeWingPaletteRenderingVerified",false);
        state.addProperty("nativeCapeTintReplayVerified",false);
        state.addProperty("layerDitheringTrackAvailable",hasLayerDithering());state.addProperty("nativeCapeDitheringReplayVerified",false);
        state.addProperty("layerShaderInputsTrackAvailable",hasLayerShaderInputs());state.addProperty("layerMaterialContextTrackAvailable",hasLayerMaterialContext());
        state.addProperty("layerMaterialPathsTrackAvailable",hasLayerMaterialPaths());state.addProperty("layerMaterialVariationTrackAvailable",hasLayerMaterialVariation());
        state.addProperty("nativeLayerLightingReplayVerified",false);
        state.addProperty("attachmentInterpolation","polar_affine");
        state.addProperty("attachmentInterpolationVerified",false);state.addProperty("attachmentCoverageVerified",false);
        state.addProperty("wingInterpolationVerified",false);
        int equipment=0;for(Actor actor:selected.actors.values())if(actor.bedrockEquipment!=null)equipment++;
        state.addProperty("equipmentObservedPlayers",equipment);state.addProperty("lastEquipmentMismatch",equipmentHistoryError);
        int nativeSkins=0,nativeSkinStates=0,nativeCapes=0,nativeCapeStates=0;for(Actor actor:selected.actors.values()) {
            if(actor.nativeSkinCaptured)nativeSkinStates++;if(actor.bedrockSkin!=null)nativeSkins++;
            if(actor.nativeCapeCaptured)nativeCapeStates++;if(actor.bedrockCape!=null&&actor.bedrockCape.present)nativeCapes++;
        }
        state.addProperty("nativeSkinStatesRecorded",nativeSkinStates);state.addProperty("nativeSkinPixelsRecorded",nativeSkins);
        state.addProperty("nativeCapeStatesRecorded",nativeCapeStates);state.addProperty("nativeCapePixelsRecorded",nativeCapes);
        state.addProperty("nativeCapeRenderingVerified",false);
        if(association!=null) {state.add("recordingId",association.get("recordingId"));state.add("alignment",association.get("alignment"));state.addProperty("alignmentVerified",false);}
        state.addProperty("events",timeline==null?1:timeline.size());state.addProperty("players",selected.actors.size());state.addProperty("models",selected.models.size());return state;
    }
}
