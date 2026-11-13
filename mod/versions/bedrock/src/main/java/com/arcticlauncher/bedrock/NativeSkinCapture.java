package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.AppearanceAssets;
import com.arcticlauncher.client.looks.AppearanceSnapshot;
import com.google.gson.JsonObject;
import java.awt.image.BufferedImage;
import java.io.*;
import java.lang.ref.WeakReference;
import java.util.*;
import javax.imageio.ImageIO;

/** Capture loaded native skin/cape pixels, independently of the public Looks
 * catalog. One instance belongs to one take; no account or native heap identity
 * is serialized. Unknown/unloaded native images remain explicit unknowns. */
final class NativeSkinCapture {
    private final AppearanceAssets assets;
    private final LinkedHashMap<String,Entry> cached=new LinkedHashMap<String,Entry>(128,.75f,true);
    private static final class Entry {
        final WeakReference<BufferedImage> image;final AppearanceAssets.Ref ref;
        Entry(BufferedImage image,AppearanceAssets.Ref ref) {this.image=new WeakReference<BufferedImage>(image);this.ref=ref;}
    }
    NativeSkinCapture(AppearanceAssets assets) {this.assets=Objects.requireNonNull(assets);}
    AppearanceSnapshot.Captured attach(AppearanceSnapshot.Captured logical,WorldFrame world,BedrockPlatform platform) throws IOException {
        Map<UUID,JsonObject> actors=new HashMap<UUID,JsonObject>();
        Map<UUID,JsonObject> capes=new HashMap<UUID,JsonObject>();
        Set<AppearanceAssets.Ref> files=new TreeSet<AppearanceAssets.Ref>();
        for(WorldFrame.Player player:world.players) {
            UUID id=UUID.fromString(player.id);
            String cape=player.nativeCape;
            if(cape==null&&player.cape!=null&&player.cape.matches("engine:cape/[0-9a-f]{64}"))cape=player.cape;
            if(cape==null&&player.nativeCapeAbsent) {
                JsonObject state=new JsonObject();state.addProperty("present",false);capes.put(id,state);
            } else if(cape!=null&&cape.matches("engine:cape/[0-9a-f]{64}")) {
                BufferedImage image=platform.texture(cape);
                if(image!=null) {
                    if(image.getWidth()<64||image.getWidth()>1024||image.getWidth()%64!=0||image.getHeight()*2!=image.getWidth())
                        throw new IOException("native replay cape is not a standard single-frame cape");
                    Entry entry=retain(cape,image);JsonObject state=new JsonObject();
                    state.addProperty("present",true);state.addProperty("texture",entry.ref.hash);
                    capes.put(id,state);files.add(entry.ref);
                }
            }
            String key=player.nativeSkin;Boolean slim=player.nativeSlim;
            if(key==null&&player.skin!=null&&player.skin.matches("engine:skin/[0-9a-f]{64}")) {key=player.skin;slim=player.slim;}
            if(key==null||slim==null||!key.matches("engine:skin/[0-9a-f]{64}"))continue;
            BufferedImage image=platform.texture(key);if(image==null)continue;
            if(image.getWidth()<64||image.getWidth()>1024||image.getWidth()%64!=0||image.getHeight()!=image.getWidth())
                throw new IOException("native replay skin is not a prepared standard skin");
            Entry entry=retain(key,image);
            JsonObject state=new JsonObject();state.addProperty("texture",entry.ref.hash);state.addProperty("slim",slim);
            actors.put(id,state);files.add(entry.ref);
        }
        return logical.withPlayerData("bedrockSkin",actors,files).withPlayerData("bedrockCape",capes);
    }
    private Entry retain(String key,BufferedImage image) throws IOException {
        Entry entry=cached.get(key);
        if(entry==null||entry.image.get()!=image) {
            if(key.startsWith("engine:cape/")&&!key.equals(NativeImageIdentity.capeKey(image)))
                throw new IOException("loaded native cape pixels differ from their content identity");
            // engineImage already owns these pixels. Native cape pixels must
            // remain raw: synthesizing elytra art would alter recorded alpha.
            ByteArrayOutputStream output=new ByteArrayOutputStream();
            if(!ImageIO.write(image,"png",output))throw new IOException("native appearance PNG writer is unavailable");
            entry=new Entry(image,assets.put("texture",output.toByteArray()));
            if(cached.size()>=256&&!cached.containsKey(key))cached.remove(cached.keySet().iterator().next());
            cached.put(key,entry);
        }
        return entry;
    }
}
