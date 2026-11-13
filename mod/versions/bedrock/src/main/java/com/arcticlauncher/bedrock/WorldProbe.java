package com.arcticlauncher.bedrock;

import com.google.gson.Gson;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.awt.image.BufferedImage;
import java.awt.image.DataBufferInt;
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import javax.imageio.ImageIO;

/** Opt-in isolated-game evidence. Never supplies camera, pose, textures or depth. */
final class WorldProbe {
    private static final Gson GSON=new Gson();
    private static final Path DIRECTORY=directory();
    private static final ProbeBodyControl BODY_CONTROL=new ProbeBodyControl(DIRECTORY,bodyProbe());
    private static final ProbeEmoteControl EMOTE_CONTROL=new ProbeEmoteControl(DIRECTORY,bodyProbe());
    private static long lastSequence;
    private static boolean bodyProbe() {
        if(DIRECTORY==null)return false;
        try {
            Path config=DIRECTORY.resolve("arctic-offline-probe.json");
            if(Files.size(config)>4096)return false;
            JsonObject input=new JsonParser().parse(new String(Files.readAllBytes(config),StandardCharsets.UTF_8)).getAsJsonObject();
            return input.get("schema").getAsInt()==1 && input.get("body_probe").getAsBoolean()
                && input.get("auto_world").getAsBoolean() && input.get("empty_user_provider").getAsBoolean()
                && "loopback-and-deny-user-provider".equals(input.get("mode").getAsString());
        }catch(Exception ignored){return false;}
    }
    static ProbeBodyControl.State bodyControl(){return BODY_CONTROL.poll();}
    static void pollEmote(BedrockPlatform platform){EMOTE_CONTROL.poll(platform);}
    static synchronized void log(String message) {
        if(DIRECTORY==null)return;
        try {
            Path path=DIRECTORY.resolve("probe-client.log");
            long size=Files.exists(path)?Files.size(path):0;
            if(size<1_048_576) {
                String line=message.length()>4096?message.substring(0,4096):message;
                byte[] bytes=(line+System.lineSeparator()).getBytes(StandardCharsets.UTF_8);
                int remaining=(int)(1_048_576-size);
                Files.write(path,bytes.length>remaining?Arrays.copyOf(bytes,remaining):bytes,
                    StandardOpenOption.CREATE,StandardOpenOption.APPEND);
            }
        } catch(Exception ignored) {}
    }
    private static Path directory() {
        String value=System.getProperty("arctic.bedrock.probe.directory");
        if(value==null)return null;
        Path path=Paths.get(value);
        return path.isAbsolute()&&Files.isDirectory(path)?path:null;
    }
    static long capture(WorldFrame world,BufferedImage image,BedrockPlatform platform) {
        if(DIRECTORY==null)return 0;
        Path request=DIRECTORY.resolve("probe-world.request.json");
        if(!Files.exists(request))return 0;
        try {
            if(Files.size(request)>4096)throw new IllegalArgumentException("world probe request exceeds limit");
            JsonObject input=new JsonParser().parse(new String(Files.readAllBytes(request),StandardCharsets.UTF_8)).getAsJsonObject();
            long sequence=input.get("sequence").getAsLong();
            if(sequence<=lastSequence||sequence>1_000_000_000)throw new IllegalArgumentException("world probe sequence is stale or out of range");
            lastSequence=sequence;
            Files.delete(request);
            String name="probe-world-"+sequence;
            Path temporary=DIRECTORY.resolve(name+".png.tmp");
            if(!ImageIO.write(image,"png",temporary.toFile()))throw new IllegalStateException("PNG writer unavailable");
            Files.move(temporary,DIRECTORY.resolve(name+".png"),StandardCopyOption.REPLACE_EXISTING);
            int[] pixels=image.getRaster().getDataBuffer() instanceof DataBufferInt
                ?((DataBufferInt)image.getRaster().getDataBuffer()).getData()
                :image.getRGB(0,0,image.getWidth(),image.getHeight(),null,0,image.getWidth());
            int count=0,left=image.getWidth(),top=image.getHeight(),right=-1,bottom=-1;
            for(int i=0;i<pixels.length;i++)if((pixels[i]>>>24)!=0) {
                count++;int x=i%image.getWidth(),y=i/image.getWidth();
                left=Math.min(left,x);top=Math.min(top,y);right=Math.max(right,x);bottom=Math.max(bottom,y);
            }
            Map<String,Object> report=new LinkedHashMap<>();
            report.put("schema",1);report.put("sequence",sequence);report.put("file",name+".png");
            report.put("world",world);report.put("pixels",count);
            if(input.has("depth")&&input.get("depth").getAsBoolean()) {
                float[] values=world.depthSnapshot();
                java.nio.ByteBuffer bytes=java.nio.ByteBuffer.allocate(values.length*4).order(java.nio.ByteOrder.LITTLE_ENDIAN);
                bytes.asFloatBuffer().put(values);
                Path depthTemp=DIRECTORY.resolve(name+".depth.f32.tmp");
                Files.write(depthTemp,bytes.array());
                String depthName=name+".depth.f32";
                Files.move(depthTemp,DIRECTORY.resolve(depthName),StandardCopyOption.REPLACE_EXISTING);
                report.put("depthFile",depthName);report.put("depthFormat","f32le");
            }
            ProbeBodyControl.State control=bodyControl();
            report.put("bodyControlMode",control.mode);report.put("bodyControlAlpha",control.alpha);
            report.put("bodyControlMethod",control.method);
            report.put("bounds",count==0?null:new int[]{left,top,right+1,bottom+1});
            report.put("loadedCosmetics",new TreeSet<>(platform.cosmetics.keySet()));
            report.put("loadedTextures",new TreeSet<>(platform.textures.keySet()));
            if(platform.replayAppearanceActive())report.put("recordedAppearance",platform.replayAppearanceStatus());
            if(!platform.replayAppearanceActive()&&com.arcticlauncher.client.ArcticClient.looks()!=null) {
                Map<String,String> capes=new LinkedHashMap<>();
                for(WorldFrame.Player player:world.players) {
                    com.arcticlauncher.client.looks.Look look=com.arcticlauncher.client.ArcticClient.looks().lookFor(UUID.fromString(player.id));
                    if(look!=null&&look.cape!=null&&com.arcticlauncher.client.ArcticClient.looks().isReady(look.cape))
                        capes.put(player.id,"look:"+com.arcticlauncher.client.ArcticClient.looks().frameAt(look.cape,world.cosmeticTimeMillis()));
                }
                report.put("capeTextures",capes);
                Map<String,Object> emotes=new LinkedHashMap<>();
                for(WorldFrame.Player player:world.players) {
                    com.arcticlauncher.client.looks.Cosmetics.Playing playing=com.arcticlauncher.client.ArcticClient.looks()
                        .cosmetics().playingFor(UUID.fromString(player.id));
                    if(playing!=null) {
                        Map<String,Object> sample=new LinkedHashMap<>();float seconds=playing.seconds();
                        sample.put("id",playing.emote.id);sample.put("seconds",seconds);
                        sample.put("leftArm",playing.emote.animation.rotation("leftArm",seconds));
                        emotes.put(player.id,sample);
                    }
                }
                report.put("emotePlayback",emotes);
                report.put("looksUrl",com.arcticlauncher.client.ArcticClient.looks().baseUrl());
                report.put("lookDiagnostics",com.arcticlauncher.client.ArcticClient.looks().diagnostics());
            }
            report.put("cosmeticsParityVerified",false);
            Path metadata=DIRECTORY.resolve(name+".json.tmp");
            Files.write(metadata,GSON.toJson(report).getBytes(StandardCharsets.UTF_8));
            Files.move(metadata,DIRECTORY.resolve(name+".json"),StandardCopyOption.REPLACE_EXISTING);
            return sequence;
        } catch(Exception error) {
            try {
                Files.deleteIfExists(request);
                Files.write(DIRECTORY.resolve("probe-world-error.json"),GSON.toJson(Collections.singletonMap("message",error.toString())).getBytes(StandardCharsets.UTF_8));
            } catch(Exception ignored) {}
            return 0;
        }
    }
}
