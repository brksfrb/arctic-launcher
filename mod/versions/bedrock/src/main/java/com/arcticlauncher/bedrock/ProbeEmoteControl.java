package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.*;
import com.google.gson.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;

/** Render-thread commands for an explicitly enabled isolated body probe. */
final class ProbeEmoteControl {
    private final Path directory;
    private final boolean enabled;
    private long lastSequence;
    ProbeEmoteControl(Path directory,boolean enabled){this.directory=directory;this.enabled=enabled;}
    void poll(BedrockPlatform platform) {
        if(!enabled||directory==null)return;
        Path request=directory.resolve("probe-emote-control.json");
        if(!Files.exists(request))return;
        long sequence=0;
        try {
            if(Files.size(request)>4096)throw new IllegalArgumentException("emote control exceeds limit");
            JsonObject input=new JsonParser().parse(new String(Files.readAllBytes(request),StandardCharsets.UTF_8)).getAsJsonObject();
            sequence=input.get("sequence").getAsLong();
            if(input.get("schema").getAsInt()!=1||sequence<=0||sequence>1_000_000_000)
                throw new IllegalArgumentException("invalid emote control sequence/schema");
            if(sequence<=lastSequence)return;
            String action=input.get("action").getAsString();
            if(!"play".equals(action)&&!"stop".equals(action))throw new IllegalArgumentException("invalid emote action");
            Looks looks=ArcticClient.looks();
            if(platform==null||platform.worldPlayerId()==null||looks==null)return;
            Cosmetics.Emote emote=null;
            if("play".equals(action)) {
                emote=looks.cosmetics().emote("arctic_probe_emote");
                if(emote==null||looks.cosmetics().animation(emote)==null)return;
            }
            looks.playEmote(emote);
            JsonObject ack=new JsonObject();ack.addProperty("schema",1);ack.addProperty("sequence",sequence);
            ack.addProperty("action",action);ack.addProperty("player",platform.worldPlayerId().toString());
            acknowledge(ack);lastSequence=sequence;
        }catch(Exception error) {
            JsonObject ack=new JsonObject();ack.addProperty("schema",1);ack.addProperty("sequence",sequence);
            ack.addProperty("error","emote control rejected: "+error.getClass().getSimpleName());
            try{acknowledge(ack);if(sequence>0&&sequence<=1_000_000_000)lastSequence=Math.max(lastSequence,sequence);}
            catch(Exception ignored){}
        }
    }
    private void acknowledge(JsonObject ack)throws Exception {
        Path temp=directory.resolve("probe-emote-control-result.json.tmp");
        Files.write(temp,ack.toString().getBytes(StandardCharsets.UTF_8));
        Files.move(temp,directory.resolve("probe-emote-control-result.json"),StandardCopyOption.REPLACE_EXISTING);
    }
}
