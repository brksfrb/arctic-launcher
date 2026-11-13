package com.arcticlauncher.client.looks;

import com.google.gson.JsonArray;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import java.io.File;
import java.io.IOException;
import java.util.*;

/** A portable point in an appearance timeline. It contains explicit null looks
 * and emotes, so a later reader need not ask the current public catalog. */
public final class AppearanceSnapshot {
    private AppearanceSnapshot() {}

    /** Capture logical appearance on the client thread. No asset downloads are
     * performed here: required files must already have been loaded and cached. */
    public static void write(Looks looks,AppearanceAssets assets,Collection<UUID> players,UUID localPlayer,
            String worldSession,long timeMillis,boolean reduceMotion,File output) throws IOException {
        Captured captured=capture(looks,players,localPlayer,worldSession,timeMillis,reduceMotion,false);
        if(assets==null)throw new IOException("appearance asset cache is unavailable");
        assets.archive(output,captured.state,captured.files);
    }

    /** Owned logical state and references; raw asset I/O can run on a writer.
     * The change key excludes advancing emote phase, but includes its stable
     * start and held phase so restarting the same emote is a distinct event. */
    public static final class Captured {
        public final JsonObject state;
        public final SortedSet<AppearanceAssets.Ref> files;
        public final String changeKey;
        private final JsonObject starts;
        private Captured(JsonObject state,SortedSet<AppearanceAssets.Ref> files,JsonObject starts) {
            this.state=state;this.files=Collections.unmodifiableSortedSet(files);
            this.starts=new com.google.gson.JsonParser().parse(starts.toString()).getAsJsonObject();
            JsonObject key=new com.google.gson.JsonParser().parse(state.toString()).getAsJsonObject();key.remove("timeMs");key.remove("cosmeticTimeMs");
            for(com.google.gson.JsonElement value:key.getAsJsonArray("players")) {
                JsonObject actor=value.getAsJsonObject();
                if(!actor.get("emote").isJsonNull()) {
                    JsonObject emote=actor.getAsJsonObject("emote");
                    if(!emote.get("held").getAsBoolean()){emote.remove("seconds");emote.remove("elapsedMillis");}
                    emote.add("since",starts.get(actor.get("id").getAsString()));
                }
            }
            changeKey=key.toString();
        }
        /** Attach bounded adapter observations to existing actors. The copied
         * values participate in change detection, including explicit unknown
         * states, while advancing emote phase retains its original start key. */
        public Captured withPlayerData(String name,Map<UUID,JsonObject> values) throws IOException {
            return withPlayerData(name,values,Collections.<AppearanceAssets.Ref>emptyList());
        }
        /** Adapter state can reference already retained, content-addressed files.
         * The archive writer still checks and copies every referenced byte. */
        public Captured withPlayerData(String name,Map<UUID,JsonObject> values,Collection<AppearanceAssets.Ref> references) throws IOException {
            if(name==null||!name.matches("[a-z][A-Za-z0-9]{0,63}")||values==null||values.size()>128)
                throw new IOException("invalid appearance adapter state");
            if(references==null||references.size()>4096)throw new IOException("invalid appearance adapter files");
            for(AppearanceAssets.Ref ref:references)if(ref==null)throw new IOException("invalid appearance adapter file");
            JsonObject copy=new com.google.gson.JsonParser().parse(state.toString()).getAsJsonObject();
            Set<UUID> remaining=new HashSet<UUID>(values.keySet());
            for(com.google.gson.JsonElement value:copy.getAsJsonArray("players")) {
                JsonObject actor=value.getAsJsonObject();UUID id=UUID.fromString(actor.get("id").getAsString());
                if(actor.has(name))throw new IOException("appearance adapter field already exists: "+name);
                JsonObject data=values.get(id);remaining.remove(id);
                if(data==null)actor.add(name,JsonNull.INSTANCE);
                else {
                    String json=data.toString();
                    if(json.getBytes(java.nio.charset.StandardCharsets.UTF_8).length>8192)throw new IOException("appearance actor adapter state exceeds 8 KiB");
                    actor.add(name,new com.google.gson.JsonParser().parse(json));
                }
            }
            if(!remaining.isEmpty())throw new IOException("appearance adapter state names an absent actor");
            SortedSet<AppearanceAssets.Ref> retained=new TreeSet<AppearanceAssets.Ref>(files);retained.addAll(references);
            if(retained.size()>4096)throw new IOException("appearance adapter exceeds the archive file budget");
            return new Captured(copy,retained,starts);
        }
    }

    public static Captured capture(Looks looks,Collection<UUID> players,UUID localPlayer,
            String worldSession,long timeMillis,boolean reduceMotion,boolean drawableOnly) throws IOException {
        return capture(looks,players,localPlayer,worldSession,timeMillis,timeMillis,reduceMotion,drawableOnly);
    }
    public static Captured capture(Looks looks,Collection<UUID> players,UUID localPlayer,
            String worldSession,long timeMillis,long cosmeticTimeMillis,boolean reduceMotion,boolean drawableOnly) throws IOException {
        if(looks==null||players==null||players.size()>128||worldSession==null
                || !worldSession.matches("[A-Za-z0-9_-]{1,64}")||timeMillis<0||timeMillis>86_400_000||cosmeticTimeMillis<0||cosmeticTimeMillis>1_000_000_000_000L)
            throw new IOException("appearance snapshot needs a current world, at most 128 players and a replay timestamp");
        SortedSet<UUID> ids=new TreeSet<UUID>(players);
        if(localPlayer!=null&&!ids.contains(localPlayer))throw new IOException("appearance local player is absent from the snapshot");
        JsonObject manifest=new JsonObject();manifest.addProperty("format",1);manifest.addProperty("rig",2);
        manifest.addProperty("session",worldSession);manifest.addProperty("timeMs",timeMillis);
        manifest.addProperty("cosmeticTimeMs",cosmeticTimeMillis);
        manifest.addProperty("localPlayer",localPlayer==null?null:localPlayer.toString());
        manifest.addProperty("reduceMotion",reduceMotion);
        JsonArray actors=new JsonArray();manifest.add("players",actors);
        JsonObject starts=new JsonObject();
        SortedSet<AppearanceAssets.Ref> files=new TreeSet<AppearanceAssets.Ref>();
        SortedMap<String,JsonObject> items=new TreeMap<String,JsonObject>(), emotes=new TreeMap<String,JsonObject>();
        Cosmetics cosmetics=looks.cosmetics();
        for(UUID id:ids) {
            JsonObject actor=new JsonObject();actor.addProperty("id",id.toString());actors.add(actor);
            Look look=looks.lookForSnapshot(id);
            if(look==null)actor.add("look",JsonNull.INSTANCE);
            else {
                String skin=drawableOnly&&look.skin!=null&&!looks.isReady(look.skin)?null:look.skin;
                String cape=drawableOnly&&look.cape!=null&&!looks.isReady(look.cape)?null:look.cape;
                JsonObject value=new JsonObject();value.addProperty("skin",skin);value.addProperty("slim",look.slim);
                value.addProperty("cape",cape);value.addProperty("arctic",look.arctic);
                value.addProperty("capeFps",looks.capeRateForSnapshot(look.cape));
                if(skin!=null)texture(looks,files,skin);
                if(cape!=null)texture(looks,files,cape);
                JsonArray worn=new JsonArray();JsonObject fits=new JsonObject();value.add("cosmetics",worn);value.add("fits",fits);
                for(String itemId:look.cosmetics) {
                    Cosmetics.Item item=cosmetics.loadedItem(itemId);
                    if(drawableOnly&&(item==null||!item.ready))continue;
                    if(item==null||!item.ready)throw new IOException("cosmetic is not loaded: "+itemId);
                    JsonObject definition=item(item,files);
                    JsonObject previous=items.put(itemId,definition);
                    if(previous!=null&&!previous.toString().equals(definition.toString()))throw new IOException("cosmetic catalog changed during snapshot");
                    worn.add(new com.google.gson.JsonPrimitive(itemId));Fit fit=look.fit(itemId);
                    JsonObject position=new JsonObject();position.addProperty("up",fit.up);position.addProperty("forward",fit.forward);
                    position.addProperty("mirror",fit.mirror);fits.add(itemId,position);
                }
                actor.add("look",value);
            }
            Cosmetics.Playing playing=cosmetics.playingForSnapshot(id);
            if(playing==null)actor.add("emote",JsonNull.INSTANCE);
            else {
                Cosmetics.Playing.Phase phase=playing.sample();float seconds=phase.seconds;
                if(!Float.isFinite(seconds)||seconds<0||seconds>86_400)throw new IOException("invalid appearance emote phase");
                if(phase.elapsedMillis!=null&&(!Double.isFinite(phase.elapsedMillis)||phase.elapsedMillis<0||phase.elapsedMillis>86_400_000))
                    throw new IOException("invalid appearance emote millisecond phase");
                Cosmetics.Emote emote=playing.emote;
                starts.addProperty(id.toString(),playing.since);
                JsonObject definition=new JsonObject();definition.addProperty("id",emote.id);definition.addProperty("name",emote.name);
                definition.addProperty("animation",emote.file);files.add(new AppearanceAssets.Ref("asset",emote.file));
                JsonObject previous=emotes.put(emote.id,definition);
                if(previous!=null&&!previous.toString().equals(definition.toString()))throw new IOException("emote catalog changed during snapshot");
                JsonObject at=new JsonObject();at.addProperty("id",emote.id);at.addProperty("seconds",seconds);
                if(phase.elapsedMillis!=null)at.addProperty("elapsedMillis",phase.elapsedMillis);
                at.addProperty("held",playing.heldForSnapshot());actor.add("emote",at);
            }
        }
        JsonArray models=new JsonArray();for(JsonObject item:items.values())models.add(item);manifest.add("cosmetics",models);
        JsonArray moves=new JsonArray();for(JsonObject emote:emotes.values())moves.add(emote);manifest.add("emotes",moves);
        return new Captured(manifest,files,starts);
    }

    private static void texture(Looks looks,Set<AppearanceAssets.Ref> files,String hash) throws IOException {
        if(!looks.isReady(hash))throw new IOException("appearance texture is not loaded: "+hash);
        files.add(new AppearanceAssets.Ref("texture",hash));
    }
    private static JsonObject item(Cosmetics.Item item,Set<AppearanceAssets.Ref> files) throws IOException {
        JsonObject value=new JsonObject();value.addProperty("id",item.id);value.addProperty("name",item.name);value.addProperty("slot",item.slot);
        if(item.meshModel!=null) {
            value.addProperty("mesh",item.mesh);files.add(new AppearanceAssets.Ref("asset",item.mesh));
        } else {
            if(item.geometry==null||item.model==null||item.texture==null)throw new IOException("cosmetic has no loaded model: "+item.id);
            value.addProperty("model",item.model);value.addProperty("texture",item.texture);value.addProperty("glow",item.glow);
            value.addProperty("animation",item.animation);files.add(new AppearanceAssets.Ref("asset",item.model));
            files.add(new AppearanceAssets.Ref("texture",item.texture));
            if(item.glow!=null)files.add(new AppearanceAssets.Ref("texture",item.glow));
            if(item.animation!=null)files.add(new AppearanceAssets.Ref("asset",item.animation));
        }
        return value;
    }
}
