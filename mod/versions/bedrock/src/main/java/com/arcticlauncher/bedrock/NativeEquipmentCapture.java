package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.*;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.*;

/** Equipment payloads live in checked assets rather than the 8 KiB logical
 * actor adapter field. References participate in appearance change detection. */
final class NativeEquipmentCapture {
    static final int MAX_BYTES=64*1024;
    private AppearanceAssets owner;
    private final LinkedHashMap<String,AppearanceAssets.Ref> cached=new LinkedHashMap<String,AppearanceAssets.Ref>(256,.75f,true);
    synchronized AppearanceSnapshot.Captured attach(AppearanceSnapshot.Captured logical,Map<UUID,JsonObject> equipment,AppearanceAssets assets) throws IOException {
        if(equipment==null||equipment.size()>128||assets==null)throw new IOException("invalid equipment capture source");
        if(owner!=assets){cached.clear();owner=assets;}
        Map<UUID,JsonObject> sources=new HashMap<UUID,JsonObject>();
        Set<AppearanceAssets.Ref> files=new TreeSet<AppearanceAssets.Ref>();
        for(Map.Entry<UUID,JsonObject> actor:equipment.entrySet()) {
            if(actor.getKey()==null||actor.getValue()==null)throw new IOException("invalid equipment capture actor");
            byte[] bytes=actor.getValue().toString().getBytes(StandardCharsets.UTF_8);
            if(bytes.length<1||bytes.length>MAX_BYTES)throw new IOException("equipment capture exceeds 64 KiB per actor");
            ReplayAppearance.validateEquipment(actor.getValue());
            String identity;
            try {identity=Base64.getEncoder().encodeToString(MessageDigest.getInstance("SHA-256").digest(bytes));}
            catch(NoSuchAlgorithmException missing){throw new IOException(missing);}
            AppearanceAssets.Ref ref=cached.get(identity);
            if(ref==null) {
                ref=assets.put("asset",bytes);
                if(cached.size()>=256)cached.remove(cached.keySet().iterator().next());
                cached.put(identity,ref);
            }
            // The history/snapshot writer still reads and verifies retained
            // bytes. Evicted/missing assets fail that write, never omit gear.
            files.add(ref);
            JsonObject source=new JsonObject();source.addProperty("schema",1);source.addProperty("hash",ref.hash);
            sources.put(actor.getKey(),source);
        }
        return logical.withPlayerData("bedrockEquipmentSource",sources,files);
    }
}
